//! The right-hand Advanced drawer (M3a spec §2.5): the preset inspector, the live-encode switch,
//! the rarely changed texture fields, and the `[files]` entries that match nothing. Drawing only: every edit is a
//! call on the model, applied once the drawing is over.
use super::columns::{strip_button, SETS_MIN, STRIP_WIDTH, TEXTURES_MIN};
use super::settings_panel::{field_row, Ctx, RowEdit};
use crate::model::fields::{advanced_field_views, levels_view, provenance_text};
use crate::model::inspector::{inspector_rows, InspectorRow};
use crate::model::{AppModel, Selection};
use crate::settings::ChecksSettings;
use eframe::egui;
use engine::derive::recipe::LegacyParams;
use engine::policy::ResolvedJob;
use serde_json::Value;

/// The drawer's narrowest width.
const ADV_MIN: f32 = 260.0;

#[derive(Debug, Default)]
pub struct AdvancedState {
    pub only_differences: bool,
}

/// What a control asks of the model.
enum Action {
    SetField(String, &'static str, Value),
    ClearField(String, &'static str),
    RemoveUnmatched(String),
    Live(bool),
    Checks(ChecksSettings),
    AlsoRecipe(LegacyParams),
}

/// The drawer: a 24 px "Advanced" strip while closed, the panel while `settings.advanced_open`.
pub fn draw(ui: &mut egui::Ui, model: &mut AppModel, state: &mut AdvancedState) {
    if !model.settings.advanced_open {
        egui::Panel::right("advanced-strip")
            .resizable(false)
            .exact_size(STRIP_WIDTH)
            .frame(egui::Frame::new().fill(ui.visuals().panel_fill))
            .show(ui, |ui| {
                if strip_button(ui, "Advanced")
                    .on_hover_text("Open the Advanced drawer")
                    .clicked()
                {
                    model.settings.advanced_open = true;
                    model.save_settings();
                }
            });
        return;
    }
    let mut close = false;
    // Drawn before the columns: it leaves them their minimums and the centre its floor (K8, rule §2.3).
    let max = ui.available_width() - TEXTURES_MIN - SETS_MIN - super::centre_floor(ui.ctx(), model);
    let panel = egui::Panel::right("advanced")
        .default_size(380.0)
        .size_range(ADV_MIN..=max.max(ADV_MIN));
    // The user's width survives the cap (a narrow window, an expanded preview) and is what gui.toml saves.
    super::chosen_width_panel(ui, "advanced", 380.0, panel, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Advanced");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                close = ui.button("Close").clicked();
            });
        });
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| body(ui, model, state));
    });
    if close {
        model.settings.advanced_open = false;
        model.save_settings();
    }
}

/// The drawer's sections (also the set panel's Advanced tab, layout v2 C6); applies its edits once they are drawn.
pub fn body(ui: &mut egui::Ui, model: &mut AppModel, state: &mut AdvancedState) {
    let mut actions = vec![];
    inspector(ui, model, state);
    preview_settings(ui, model, &mut actions);
    checks(ui, model, &mut actions);
    also_recipe(ui, model, &mut actions);
    mipmaps(ui, model, &mut actions);
    texture_fields(ui, model, &mut actions);
    unmatched(ui, model, &mut actions);
    for a in actions {
        match a {
            Action::SetField(k, f, v) => model.set_field(&k, f, v),
            Action::ClearField(k, f) => model.clear_field(&k, f),
            Action::RemoveUnmatched(k) => model.remove_unmatched(&k),
            Action::Live(on) => model.set_live(on),
            Action::Checks(c) => model.set_checks(c),
            Action::AlsoRecipe(p) => model.set_also_settings(|c| c.legacy = p),
        }
    }
}

/// The selected texture's key and resolved job, when it has one.
fn selected_job(model: &AppModel) -> Option<(String, ResolvedJob)> {
    // F3a: the drawer edits one texture; with several selected it waits for one.
    if model.in_bulk() {
        return None;
    }
    let Selection::Item(key) = &model.selection else {
        return None;
    };
    let job = model.item(key)?.job.clone()?;
    Some((key.clone(), job))
}

fn value_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn section(ui: &mut egui::Ui, name: &str, body: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(egui::RichText::new(name).strong())
        .default_open(true)
        .show(ui, body);
}

/// Slack kept off the inspector grid's width for the scroll bar and the grid's own cell padding (rule 3.7).
const GRID_MARGIN: f32 = 24.0;
/// The narrowest an inspector column gets.
const MIN_COL: f32 = 60.0;

/// A field's row text: the bulk panel's label, else the key as words ("alpha.compare" is "Alpha compare").
fn field_text(field: &str) -> String {
    let label = crate::model::bulk::field_label(field);
    if label != field {
        label.to_string()
    } else {
        crate::model::labels::readable(&field.replace('.', "_"))
    }
}

fn inspector(ui: &mut egui::Ui, model: &mut AppModel, state: &mut AdvancedState) {
    let mut open_presets = false;
    section(ui, "Preset inspector", |ui| {
        // D10: the drawer names the texture first (A18).
        // Only a texture with resolved settings is named; the body below has nothing to show for any other.
        let heading = match selected_job(model) {
            Some((key, _)) => format!(
                "How {}'s settings resolve",
                key.rsplit('/').next().unwrap_or(&key)
            ),
            None => "No texture selected".to_string(),
        };
        ui.label(egui::RichText::new(heading).strong());
        ui.weak(format!("Batch preset: {}", model.batch_preset));
        if model.loaded_user_presets.is_empty() {
            ui.horizontal(|ui| {
                ui.weak("No user presets.");
                open_presets = ui.button("Presets…").clicked();
            });
        } else {
            ui.label("User presets:");
            for (id, file) in &model.loaded_user_presets {
                ui.label(egui::RichText::new(format!("{id} — {file}")).small());
            }
        }
        for (file, reason) in &model.skipped_user_presets {
            ui.label(
                egui::RichText::new(format!("Skipped: {file} — {reason}"))
                    .small()
                    .color(ui.visuals().warn_fg_color),
            );
        }
        ui.label(
            egui::RichText::new(
                "Built-in presets are read-only here. Change a texture in the centre panel.",
            )
            .small()
            .weak(),
        );
        let (Some((_, job)), Some(session)) = (selected_job(model), model.session.as_ref()) else {
            ui.weak("Select a texture to see how its settings were resolved.");
            return;
        };
        ui.checkbox(
            &mut state.only_differences,
            format!("Show only what differs from {}", engine::presets::BASE_ID),
        );
        let rows: Vec<InspectorRow> = inspector_rows(&job, session.engine().registry())
            .into_iter()
            .filter(|r| !state.only_differences || r.differs_from_base)
            .collect();
        if rows.is_empty() {
            ui.weak("Nothing differs from the base preset.");
            return;
        }
        // Three columns share the drawer's width (the drawer's minimum keeps each above 60 px, so no side scroll).
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
                    key_label.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &name)
                    });
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
    });
    if open_presets {
        model.open_presets();
    }
}

/// A number box named `name`; true once its value is settled and differs from what came in: a drag ended, or typed
/// text was accepted by leaving the box (Enter, Tab or a click away; Esc keeps the old value). Typing "200" is one
/// edit, not three (Note FN2): the text is applied only when the box loses focus (`update_while_editing(false)`).
/// The in-progress value of a drag, or of arrow keys on the focused box, is held in egui memory, so a caller that
/// rebuilds `value` from the model each frame still gets the final value on release or when the box is left. While
/// `busy` (a re-plan is out) a settled value stays on show until the model hands it back (Backlog R2); `scope` (the
/// texture or set the box edits) keys the held values, so they never show in another item's box. The match with what
/// came in is exact `f64` equality: a caller whose model rounds the value would hold it until the plan is idle.
pub(super) fn settled_drag<T: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    value: &mut T,
    speed: f64,
    range: std::ops::RangeInclusive<T>,
    name: &str,
    busy: bool,
    scope: &str,
) -> bool {
    let id = ui.next_auto_id().with(("settled_drag", scope));
    let came_in = *value;
    let mut shown = came_in;
    let (out, _) = held(ui, id, came_in, busy, true, |ui, v| {
        let r = ui.add(
            egui::DragValue::new(v)
                .speed(speed)
                .range(range)
                .update_while_editing(false),
        );
        let x = v.to_f64();
        r.widget_info(|| {
            let mut i = egui::WidgetInfo::drag_value(r.enabled(), x);
            i.label = Some(name.to_string());
            i
        });
        shown = *v;
        r
    });
    *value = shown;
    out.is_some()
}

/// A widget over a model value that is written once per gesture (L29): the value in flight, and one already sent,
/// live in egui memory under `id`, so a caller that rebuilds `current` from the model each frame still sees the
/// final value. `add` draws the widget on a copy of `current` and returns its response. `Some(value)` on the frame it
/// settles and differs from `current`: a drag ended, or typed text was accepted by leaving the box (Enter, Tab or a
/// click away; Esc keeps the old value), so "200" is one edit, not three (Note FN2) and an arrow key on a focused box
/// is one too (`on_blur`: a number box, whose key presses settle when it is left). A slider (`on_blur` false) settles
/// on each key press, so arrow keys edit live. While `busy` (a re-plan is out) a settled value stays on show until the
/// model hands it back (Backlog R2). The match with `current` is exact `f64` equality: a caller whose model rounds the
/// value would hold it until the plan is idle.
pub(super) fn held<T: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    id: egui::Id,
    current: T,
    busy: bool,
    on_blur: bool,
    add: impl FnOnce(&mut egui::Ui, &mut T) -> egui::Response,
) -> (Option<T>, egui::Response) {
    let cur = current.to_f64();
    let mut pending: Option<(f64, bool)> = ui.data(|d| d.get_temp(id));
    if pending.is_some_and(|(v, sent)| v == cur || (sent && !busy)) {
        pending = None;
    }
    let mut x = pending.map_or(current, |p| T::from_f64(p.0));
    let r = add(ui, &mut x);
    if r.changed() {
        pending = Some((x.to_f64(), false));
    }
    let settled = r.drag_stopped()
        || r.lost_focus()
        || (r.changed() && !r.dragged() && !(on_blur && r.has_focus()));
    let mut out = None;
    if let Some((pv, false)) = pending {
        if settled && pv != cur {
            out = Some(T::from_f64(pv));
            pending = Some((pv, true));
        }
    }
    ui.data_mut(|d| match pending {
        Some(p) => {
            d.insert_temp(id, p);
        }
        None => d.remove::<(f64, bool)>(id),
    });
    (out, r)
}

fn preview_settings(ui: &mut egui::Ui, model: &AppModel, actions: &mut Vec<Action>) {
    section(ui, "Preview", |ui| {
        let mut live = model.settings.live_encode;
        if ui
            .checkbox(&mut live, "Live encode")
            .on_hover_text(
                "Encode the expanded preview after every edit (otherwise use Encode now)",
            )
            .changed()
        {
            actions.push(Action::Live(live));
        }
    });
}

/// The artist checks' switches (DECISIONS Note CL). Each box's AccessKit name ends in "check", apart from same-text
/// controls such as the shader's "Parallax" (K22, ui-rules 8.4).
fn checks(ui: &mut egui::Ui, model: &AppModel, actions: &mut Vec<Action>) {
    section(ui, "Checks", |ui| {
        let now = model.settings.checks;
        let mut c = now;
        let boxes: [(&mut bool, &str, &str); 3] = [
            (&mut c.low_detail, "Low detail", "Low detail check"),
            (&mut c.green, "Green direction", "Green direction check"),
            (&mut c.parallax, "Parallax", "Parallax check"),
        ];
        for (on, text, name) in boxes {
            let r = ui.checkbox(on, text);
            let v = *on;
            r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, v, name));
        }
        if c != now {
            actions.push(Action::Checks(c));
        }
    });
}

/// A number widget over a model value that is only written once it settles: the value in flight lives in egui
/// memory under `id` while it is dragged, so the drag follows the pointer and ends in exactly one edit (one undo
/// step). `Some(value)` on the frame it settles (a drag ended, a click, a typed value accepted).
pub(super) fn held_value(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    current: f64,
    add: impl FnOnce(&mut egui::Ui, &mut f64) -> egui::Response,
) -> (egui::Response, Option<f64>) {
    let (out, r) = held(ui, egui::Id::new(id), current, false, false, add);
    (r, out)
}

const OPEN_RECIPE: &str = "advanced-open-recipe";

/// Open "Also export recipe" the next time it is drawn (Review's link).
pub fn open_recipe(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(OPEN_RECIPE), true));
}

/// "Also export recipe" (owner answer 5, C4): one number box per `[export.legacy]` value, ranges as
/// `LegacyParams::check`. P17: Keep metal brightness and its strength are only in Review (with the CM tick), and
/// Dielectric env is one box (0 = off), not a box plus a checkbox.
fn also_recipe(ui: &mut egui::Ui, model: &AppModel, actions: &mut Vec<Action>) {
    // Review M4 (ruled): closed until asked for; Review's "Recipe settings are in Advanced" opens it.
    let asked = ui
        .ctx()
        .data_mut(|d| d.remove_temp::<bool>(egui::Id::new(OPEN_RECIPE)))
        .is_some();
    let header = egui::CollapsingHeader::new(egui::RichText::new("Also export recipe").strong())
        .default_open(false)
        .open(asked.then_some(true));
    header.show(ui, |ui| {
        let off = if model.project.is_none() {
            Some("Open a folder first".to_string())
        } else if model.export.running.is_some() {
            Some(crate::model::also::EXPORT_RUNNING.to_string())
        } else {
            model.toml_read_only_reason()
        };
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
                    // The number box first: in a narrow drawer the text is cut, never the box.
                    for (v, label, key, lo, hi, hover) in rows {
                        let (r, done) = held_value(ui, ("also-recipe", key), *v, |ui, x| {
                            ui.add(
                                egui::DragValue::new(x)
                                    .speed(0.01)
                                    .range(lo..=hi)
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
                        r.on_hover_ui(tip);
                        ui.horizontal(|ui| {
                            ui.label(label);
                            ui.label(egui::RichText::new(key).small().weak());
                        })
                        .response
                        .on_hover_ui(tip);
                        ui.end_row();
                    }
                });
        })
        .response
        .on_disabled_hover_text(off.as_deref().unwrap_or_default());
        if settled && p != now {
            actions.push(Action::AlsoRecipe(p));
        }
    });
}

/// The mip level count (Full chain, None or 2..N levels); the basic panel only has the on/off tick box.
fn mipmaps(ui: &mut egui::Ui, model: &AppModel, actions: &mut Vec<Action>) {
    section(ui, "Mipmaps", |ui| {
        let Some((key, job)) = selected_job(model) else {
            ui.weak("Select a texture to edit its mip level count.");
            return;
        };
        // Backlog T32 (Note FK): off while a plan is pending, as the settings panel's rows are.
        let why = model.fix_off();
        let f = levels_view(model.target, &job);
        ui.add_enabled_ui(why.is_none(), |ui| {
            egui::Grid::new(("advanced-mips", &key))
                .num_columns(3)
                .show(ui, |ui| {
                    let cx = Ctx {
                        key: &key,
                        job: &job,
                        target: model.target,
                        busy: model.edit_busy(),
                        compact: false,
                        edit_off: why.clone(),
                        warnings: &[],
                    };
                    match field_row(ui, &cx, &f) {
                        Some(RowEdit::Set(v)) => {
                            actions.push(Action::SetField(key.clone(), f.field, v))
                        }
                        Some(RowEdit::Clear) => {
                            actions.push(Action::ClearField(key.clone(), f.field))
                        }
                        None => {}
                    }
                });
        });
    });
}

fn texture_fields(ui: &mut egui::Ui, model: &AppModel, actions: &mut Vec<Action>) {
    section(ui, "Advanced texture fields", |ui| {
        let Some((key, job)) = selected_job(model) else {
            ui.weak("Select a texture to edit its encoder and header fields.");
            return;
        };
        let busy = model.edit_busy();
        let why = model.fix_off();
        ui.add_enabled_ui(why.is_none(), |ui| {
            egui::Grid::new(("advanced-fields", &key))
                .num_columns(3)
                .show(ui, |ui| {
                    let cx = Ctx {
                        key: &key,
                        job: &job,
                        target: model.target,
                        busy,
                        compact: false,
                        edit_off: why.clone(),
                        warnings: &[],
                    };
                    for f in advanced_field_views(model.target, &job) {
                        match field_row(ui, &cx, &f) {
                            Some(RowEdit::Set(v)) => {
                                actions.push(Action::SetField(key.clone(), f.field, v))
                            }
                            Some(RowEdit::Clear) => {
                                actions.push(Action::ClearField(key.clone(), f.field))
                            }
                            None => {}
                        }
                    }
                });
        });
    });
}

fn unmatched(ui: &mut egui::Ui, model: &AppModel, actions: &mut Vec<Action>) {
    section(ui, "Unmatched [files] entries", |ui| {
        if model.unmatched_keys.is_empty() {
            ui.weak("Every [files] entry in sdc.toml matches a file.");
            return;
        }
        ui.label(
            egui::RichText::new(
                "These entries name no file under the current target and have no effect.",
            )
            .small()
            .weak(),
        );
        let editable = model.project.as_ref().is_some_and(|p| !p.read_only());
        for k in &model.unmatched_keys {
            ui.horizontal(|ui| {
                ui.label(k);
                if ui
                    .add_enabled(editable, egui::Button::new("Remove"))
                    .clicked()
                {
                    actions.push(Action::RemoveUnmatched(k.clone()));
                }
            });
        }
    });
}

/// Gesture drivers for the number-box wrappers over [`held`] (L29): a typed value and an arrow key each commit once.
#[cfg(test)]
pub(super) mod gesture {
    use eframe::egui::{self, accesskit::Role};
    use egui_kittest::kittest::Queryable as _;

    /// The model value the widget shows, and what it has committed.
    pub struct St {
        pub cur: f64,
        pub commits: Vec<f64>,
    }

    impl St {
        /// The model takes a commit at once, as an idle plan does.
        pub fn take(&mut self, v: Option<f64>) {
            if let Some(v) = v {
                self.cur = v;
                self.commits.push(v);
            }
        }
    }

    fn harness(draw: fn(&mut egui::Ui, &mut St)) -> egui_kittest::Harness<'static, St> {
        let st = St {
            cur: 0.0,
            commits: vec![],
        };
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(300.0, 100.0))
            .build_ui_state(draw, st);
        h.run();
        h
    }

    /// Click the box, type `text`, press Enter: what was committed.
    pub fn typed(draw: fn(&mut egui::Ui, &mut St), role: Role, text: &str) -> Vec<f64> {
        let mut h = harness(draw);
        h.get_by_role(role).click();
        h.run();
        // The cursor opens at the start of the text ("0", "0.00" in a slider's box): Delete clears it.
        for _ in 0..6 {
            h.key_press(egui::Key::Delete);
            h.run();
        }
        h.get_by_role(role).type_text(text);
        h.run();
        h.key_press(egui::Key::Enter);
        h.run();
        h.run();
        h.state().commits.clone()
    }

    /// Focus the box and press the arrow key, without leaving it: what was committed so far.
    pub fn arrow_held(draw: fn(&mut egui::Ui, &mut St), role: Role, key: egui::Key) -> Vec<f64> {
        let mut h = harness(draw);
        h.get_by_role(role).focus();
        h.run();
        h.key_press(key);
        h.run();
        h.run();
        h.state().commits.clone()
    }

    /// Focus the box, press the arrow key, then leave it with Tab: what was committed.
    pub fn arrow(draw: fn(&mut egui::Ui, &mut St), role: Role, key: egui::Key) -> Vec<f64> {
        let mut h = harness(draw);
        h.get_by_role(role).focus();
        h.run();
        h.key_press(key);
        h.run();
        h.key_press(egui::Key::Tab);
        h.run();
        // A slider keeps the focus through Tab: a click on the draw function's "other" button takes it.
        if let Some(other) = h.query_by_label("other") {
            other.click();
        }
        h.run();
        h.run();
        h.state().commits.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::gesture::{arrow, typed, St};
    use super::*;
    use eframe::egui::accesskit::Role;

    fn settled(ui: &mut egui::Ui, s: &mut St) {
        let mut v = s.cur;
        let done = settled_drag(ui, &mut v, 1.0, 0.0..=255.0, "fill", false, "t");
        s.take(done.then_some(v));
    }

    fn held_box(ui: &mut egui::Ui, s: &mut St) {
        let (_, done) = held_value(ui, "t", s.cur, |ui, x| {
            ui.add(egui::DragValue::new(x).update_while_editing(false))
        });
        s.take(done);
    }

    #[test]
    fn settled_drag_commits_a_typed_value_and_an_arrow_key_once() {
        assert_eq!(typed(settled, Role::SpinButton, "25"), vec![25.0]);
        assert_eq!(arrow(settled, Role::SpinButton, egui::Key::ArrowUp), [1.0]);
    }

    /// A number box (`on_blur`) commits nothing while it is focused: the arrow key settles when the box is left.
    #[test]
    fn a_number_box_commits_nothing_before_it_is_left() {
        use super::gesture::arrow_held;
        assert!(arrow_held(settled, Role::SpinButton, egui::Key::ArrowUp).is_empty());
    }

    fn slider_box(ui: &mut egui::Ui, s: &mut St) {
        let (_, done) = held_value(ui, "t", s.cur, |ui, x| {
            ui.add(egui::Slider::new(x, 0.0..=100.0).show_value(false))
        });
        s.take(done);
    }

    /// FY1: a click on a slider settles it, and commits once.
    #[test]
    fn a_click_on_a_slider_commits_once() {
        use egui_kittest::kittest::Queryable as _;
        let st = St {
            cur: 0.0,
            commits: vec![],
        };
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(300.0, 100.0))
            .build_ui_state(slider_box, st);
        h.run();
        h.get_by_role(Role::Slider).click();
        h.run();
        h.run();
        let commits = &h.state().commits;
        assert_eq!(commits.len(), 1, "{commits:?}");
        assert!(commits[0] > 0.0, "{commits:?}");
    }

    #[test]
    fn held_value_commits_a_typed_value_and_an_arrow_key_once() {
        assert_eq!(typed(held_box, Role::SpinButton, "25"), vec![25.0]);
        assert_eq!(arrow(held_box, Role::SpinButton, egui::Key::ArrowUp), [1.0]);
    }
}
