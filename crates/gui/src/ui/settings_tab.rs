//! Note JI1: the Settings tab of the centre column (or its own window when popped out, Note FB7). "This project" (Mode,
//! Preset and the export folder, saved in the folder's files) comes first, then "App settings" (gui.toml: Preview,
//! Checks, Export, Cache and Columns). Drawing only: every edit is collected and applied once the tab is drawn.
use super::header::{self, target_help};
use crate::look::Combo036 as _;
use crate::model::code_text::target_label;
use crate::model::{AppModel, CentreTab, EXPORT_WITHOUT_REVIEW_WAIT};
use crate::settings::ChecksSettings;
use eframe::egui;
use engine::config::Target;

/// The view's heading and tab name.
pub const TITLE: &str = "Settings";
const PROJECT_HEADING: &str = "This project: saved in this folder's sdc.toml and sdc-project.toml";
const APP_HEADING: &str = "App settings: these apply to every project";
/// The width of the project rows' labels, so Mode, Preset and Export folder line up.
const LABEL_W: f32 = 90.0;
/// The most characters of the export path shown; the rest is cut from the middle (rule 3.4).
const PATH_CHARS: usize = 48;
/// Open's accessible name: Export to ▾ has an "Open export folder" item, and both can be on screen (rule 8.4).
const OPEN_NAME: &str = "Settings: Open export folder";

/// Note LG: the Validate exported DDS box's hover.
const VALIDATE_HOVER: &str = "Decode each DDS again after it is encoded and check it against Skyrim's rules. Takes longer. With it on, the next export also checks the files an earlier export left unchecked, without encoding them again. Complex material _m and complex grass are always checked.";

/// The lowest and highest limit the cache box offers, in MB. The prune walks the whole folder before the window
/// opens: 5,000 MB is about 150,000 entries.
const CACHE_MB: (f64, f64) = (50.0, 5_000.0);

/// What a control asks of the model.
enum Act {
    Target(Target),
    Preset(Option<String>),
    ChooseExport,
    OpenExport,
    Live(bool),
    Checks(ChecksSettings),
    ValidateExports(bool),
    CacheMb(u32),
    /// (Textures shown, Sets shown).
    Columns(bool, bool),
    ExportWithoutReview(bool),
}

/// The tab, in the centre column or in its popped-out window.
pub fn view(ui: &mut egui::Ui, model: &mut AppModel) {
    ui.horizontal(|ui| {
        ui.heading(TITLE);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            super::centre_tabs::controls(ui, model, CentreTab::Settings)
        });
    });
    let mut acts = vec![];
    egui::ScrollArea::vertical()
        .id_salt("settings-scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            project(ui, model, &mut acts);
            ui.separator();
            app(ui, model, &mut acts);
        });
    for a in acts {
        match a {
            Act::Target(t) => model.set_target(t),
            Act::Preset(p) => model.set_preset(p),
            Act::ChooseExport => {
                model.pick_export_folder(ui.ctx());
            }
            Act::OpenExport => {
                // As Export to ▾'s: the button is enabled only when the folder is there.
                let _ = model.open_export_folder();
            }
            Act::ExportWithoutReview(on) => model.set_export_without_review(on),
            Act::Live(on) => model.set_live(on),
            Act::Checks(c) => model.set_checks(c),
            Act::ValidateExports(on) => model.set_validate_exports(on),
            Act::CacheMb(mb) => model.set_analysis_cache_mb(mb),
            Act::Columns(textures, sets) => {
                model.settings.textures_hidden = !textures;
                model.settings.sets_hidden = !sets;
                model.save_settings();
            }
        }
    }
}

/// A row's label in a fixed-width slot.
fn row_label(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let size = egui::vec2(LABEL_W, ui.spacing().interact_size.y);
    let layout = egui::Layout::left_to_right(egui::Align::Center);
    ui.allocate_ui_with_layout(size, layout, |ui| {
        ui.set_min_width(LABEL_W);
        ui.add(egui::Label::new(text).show_tooltip_when_elided(false))
    })
    .inner
}

/// "This project": Mode, Preset and the export folder; with no project open, a line that says so and no controls.
fn project(ui: &mut egui::Ui, model: &mut AppModel, acts: &mut Vec<Act>) {
    ui.label(egui::RichText::new(PROJECT_HEADING).strong());
    if model.project.is_none() {
        ui.weak("No project open");
        return;
    }
    if let Some(t) = mode_row(ui, model) {
        acts.push(Act::Target(t));
    }
    if let Some(p) = preset_row(ui, model) {
        acts.push(Act::Preset(p));
    }
    export_folder_row(ui, model, acts);
    if let Some(on) = opt_out_checkbox(
        ui,
        model,
        "Export without review",
        "For this project only. Off: the header's Export opens Review first.",
    ) {
        acts.push(Act::ExportWithoutReview(on));
    }
    ui.weak(
        "Mode and Preset save with Save. The export folder saves at once unless sdc-project.toml has unsaved edits.",
    );
}

/// The project's "Export without review" box, here and in Review's footer: the text is the accessible name. Drawn off
/// with its reason while the folder's open can be cancelled (ruling 3c-13, the setter refuses then) and while
/// `sdc-project.toml` takes no edit. Returns the new value when it was toggled.
pub(super) fn opt_out_checkbox(
    ui: &mut egui::Ui,
    model: &AppModel,
    text: &str,
    hover: &str,
) -> Option<bool> {
    let off = if model.can_cancel_open() {
        Some(EXPORT_WITHOUT_REVIEW_WAIT.to_string())
    } else {
        model.sets_read_only_reason()
    };
    let mut on = model.export_without_review();
    let r = ui.add_enabled(off.is_none(), egui::Checkbox::new(&mut on, text));
    let r = match &off {
        Some(why) => r.on_disabled_hover_text(why),
        None => r.on_hover_text(hover),
    };
    r.changed().then_some(on)
}

/// Note FI16 (owner O7), moved from the Presets tab (Note JI1): the project's Mode (`target` in `sdc.toml`). The pick
/// is returned for the caller to apply. The project's target wins over a preset's own Target.
fn mode_row(ui: &mut egui::Ui, model: &AppModel) -> Option<Target> {
    let off = model.toml_read_only_reason();
    let mut picked = model.target;
    ui.horizontal(|ui| {
        row_label(ui, "Mode");
        ui.add_enabled_ui(off.is_none(), |ui| {
            let r = egui::ComboBox::from_id_salt("mode")
                .selected_text(target_label(model.target))
                .show_ui_036(ui, |ui| {
                    for t in [Target::Cs, Target::Vanilla] {
                        ui.selectable_value(&mut picked, t, target_label(t))
                            .on_hover_text(target_help(t));
                    }
                })
                .response;
            let enabled = ui.is_enabled();
            r.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::Role::ComboBox,
                    enabled,
                    format!("Mode (this project): {}", target_label(model.target)),
                )
            });
            match &off {
                Some(why) => {
                    r.on_disabled_hover_text(why);
                }
                None => {
                    r.on_hover_text(
                        "The target the textures are converted for: cs or vanilla. Saved in sdc.toml as target.",
                    );
                }
            }
        });
    });
    ui.horizontal(|ui| {
        ui.add_space(LABEL_W + ui.spacing().item_spacing.x);
        ui.weak("This beats the Target set in a preset.").on_hover_text(
            "The project's target in sdc.toml is used before the preset's Target (sdc.exe does the same).",
        );
    });
    (picked != model.target).then_some(picked)
}

/// The project's preset: the header's Preset menu (`header::preset_entries`), without Presets…. Off while sdc.toml
/// is read-only, as Mode is. The pick (`None` = the default) when it changes the preset.
fn preset_row(ui: &mut egui::Ui, model: &mut AppModel) -> Option<Option<String>> {
    let off = model.toml_read_only_reason();
    let text = header::preset_text(model);
    let mut picked = None;
    ui.horizontal(|ui| {
        row_label(ui, "Preset");
        ui.add_enabled_ui(off.is_none(), |ui| {
            let r = egui::ComboBox::from_id_salt("settings-preset")
                .selected_text(&text)
                .show_ui_036(ui, |ui| picked = header::preset_entries(ui, model))
                .response;
            let enabled = ui.is_enabled();
            r.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::Role::ComboBox,
                    enabled,
                    format!("Preset (this project): {text}"),
                )
            });
            match &off {
                Some(why) => {
                    r.on_disabled_hover_text(why);
                }
                None => {
                    r.on_hover_text(
                        "The preset the folder converts with; the list shows this mode's presets. Saved in sdc.toml as preset.",
                    );
                }
            }
        });
    });
    picked
}

/// The export folder: cut in the middle, the whole path on hover and in the accessible name (rules 3.4, 8.1), then
/// Choose… and Open, laid out first on the right so the path gives way (rule 3.3).
fn export_folder_row(ui: &mut egui::Ui, model: &AppModel, acts: &mut Vec<Act>) {
    let full = model
        .export_folder_chosen()
        .map(|p| p.display().to_string());
    let (shown, name, hover) = match &full {
        Some(f) => (
            header::elide_middle(f, PATH_CHARS),
            format!("Export folder: {f}"),
            f.clone(),
        ),
        None => (
            "Not chosen".to_string(),
            "Export folder: not chosen".to_string(),
            "No export folder chosen yet: Choose… picks one".to_string(),
        ),
    };
    // The tab's floating scroll bar senses its full width over the content's right edge: Open keeps clear of it.
    let s = &ui.spacing().scroll;
    let bar = s.bar_width + s.bar_outer_margin;
    let (choose, open) = ui
        .scope(|ui| {
            ui.set_max_width((ui.available_width() - bar).max(0.0));
            egui::Sides::new()
                .shrink_left()
                .truncate()
                .show(
                    ui,
                    |ui| {
                        let r = row_label(ui, "Export folder");
                        let enabled = ui.is_enabled();
                        r.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::Role::Label, enabled, &name)
                        });
                        r.on_hover_text(&hover);
                        ui.add(
                            egui::Label::new(&shown)
                                .truncate()
                                .show_tooltip_when_elided(false),
                        )
                        .on_hover_text(&hover);
                    },
                    // Right to left: Open first, so the row reads Choose… · Open.
                    |ui| {
                        let button = |ui: &mut egui::Ui, on: bool, text: &str, name: &str| {
                            let r = ui.add_enabled(on, egui::Button::new(text));
                            r.widget_info(|| {
                                egui::WidgetInfo::labeled(egui::Role::Button, on, name)
                            });
                            r
                        };
                        // Stat only while the tab is drawn (rule 9.4).
                        let state = model.export_folder_state().folder;
                        let r = button(ui, state.is_ok(), "Open", OPEN_NAME);
                        let open = match &state {
                            Ok(p) => r.on_hover_text(format!("Show {} in Explorer", p.display())),
                            Err(why) => r.on_disabled_hover_text(why),
                        }
                        .clicked();
                        let off = header::export_folder_off(model);
                        let r = button(ui, off.is_none(), "Choose…", "Choose export folder");
                        let choose = match off {
                            Some(why) => r.on_disabled_hover_text(why),
                            None => r.on_hover_text("Choose where converted textures are written"),
                        }
                        .clicked();
                        (choose, open)
                    },
                )
                .1
        })
        .inner;
    if choose {
        acts.push(Act::ChooseExport);
    }
    if open {
        acts.push(Act::OpenExport);
    }
}

/// "App settings": Preview, Checks, Export, Cache and Columns, each saved to gui.toml at once.
fn app(ui: &mut egui::Ui, model: &AppModel, acts: &mut Vec<Act>) {
    use super::group::group;
    ui.label(egui::RichText::new(APP_HEADING).strong());
    group(ui, "settings-preview", "Preview", None, true, |ui| {
        // Note JI1: the preview toolbar keeps its own Live encode box; this one's name says it is the setting.
        let mut live = model.settings.live_encode;
        let r = ui.checkbox(&mut live, "Live encode");
        let enabled = ui.is_enabled();
        r.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::CheckBox, enabled, live, "Live encode setting")
        });
        if r.on_hover_text(
            "Encode the expanded preview after every edit (otherwise use Encode now)",
        )
        .changed()
        {
            acts.push(Act::Live(live));
        }
    });
    group(ui, "settings-checks", "Checks", None, true, |ui| {
        // The artist checks' switches (DECISIONS Note CL). Each box's AccessKit name ends in "check", apart from
        // same-text controls such as the shader's "Parallax" (K22, ui-rules 8.4).
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
            r.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, v, name));
        }
        if c != now {
            acts.push(Act::Checks(c));
        }
    });
    group(ui, "settings-export", "Export", None, true, |ui| {
        let mut on = model.settings.validate_exports;
        let r = ui.checkbox(&mut on, "Validate exported DDS");
        let enabled = ui.is_enabled();
        r.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::CheckBox, enabled, on, "Validate exported DDS")
        });
        if r.on_hover_text(VALIDATE_HOVER).changed() {
            acts.push(Act::ValidateExports(on));
        }
    });
    group(ui, "settings-cache", "Cache", None, true, |ui| {
        if let Some(mb) = cache(ui, model) {
            acts.push(Act::CacheMb(mb));
        }
    });
    group(ui, "settings-columns", "Columns", None, true, |ui| {
        // Note BH15: which of the Textures and Sets columns are shown at all; each column's own arrow folds it to a
        // strip. Ticked = shown.
        let mut shown = [!model.settings.textures_hidden, !model.settings.sets_hidden];
        let mut changed = false;
        for (on, text) in shown.iter_mut().zip(["Textures", "Sets"]) {
            let r = ui.checkbox(on, text);
            let v = *on;
            let enabled = ui.is_enabled();
            r.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::Role::CheckBox,
                    enabled,
                    v,
                    format!("{text} column"),
                )
            });
            let verb = if v { "Hide" } else { "Show" };
            changed |= r
                .on_hover_text(format!("{verb} the {text} column"))
                .changed();
        }
        if changed {
            acts.push(Act::Columns(shown[0], shown[1]));
        }
    });
}

/// The analysis disk cache's limit (Note HV2): one number box, in MB. The cache is pruned at start only, so the hover
/// says when a new limit applies. The limit once it is settled and changed.
fn cache(ui: &mut egui::Ui, model: &AppModel) -> Option<u32> {
    const NAME: &str = "Analysis cache limit";
    let now = f64::from(
        model
            .settings
            .analysis_cache_mb
            .unwrap_or(crate::settings::ANALYSIS_CACHE_MB),
    );
    // Built only while hovered (ui-rules 9.3).
    let tip = |ui: &mut egui::Ui| {
        ui.label(
            "The most disk space the saved analyses and thumbnails of your textures may use \
             (%LOCALAPPDATA%\\sdc\\analysis-cache). They let a folder open without reading every image again. \
             Over the limit, the ones unused for longest are deleted when Tessalith starts.",
        );
        ui.label(
            "A new limit applies the next time Tessalith starts. (gui.toml analysis_cache_mb)",
        );
    };
    ui.horizontal(|ui| {
        // The number box first: in a narrow tab the text is cut, never the box.
        let mut shown = now;
        let (r, done) = super::number_input::held_value(ui, "settings-cache-mb", now, |ui, x| {
            let r = ui.add(
                egui::DragValue::new(x)
                    .speed(10.0)
                    // A limit typed into gui.toml outside the box's range stays allowed, so that looking at the box,
                    // or leaving it, never changes it.
                    .range(CACHE_MB.0.min(now)..=CACHE_MB.1.max(now))
                    .max_decimals(0)
                    .suffix(" MB")
                    .update_while_editing(false),
            );
            shown = *x;
            r
        });
        r.widget_info(|| {
            let mut i = egui::WidgetInfo::drag_value(true, shown);
            i.label = Some(NAME.to_string());
            i
        });
        r.on_hover_ui(tip);
        ui.label(NAME).on_hover_ui(tip);
        done.map(|mb| mb.round() as u32)
    })
    .inner
}
