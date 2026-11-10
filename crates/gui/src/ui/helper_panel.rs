//! Note DY17: a helper's panel (helper textures spec Goal 6): its role, the channels it fills, the channel it is read
//! from and whether it is inverted. Every value shown is the engine's `HelperReport`; the GUI only writes the keys.
use crate::model::helpers::{fill_text, target_file};
use crate::model::sets::Tails;
use crate::model::AppModel;
use eframe::egui;
use egui::text::LayoutJob;
use engine::helper::report::HelperReport;
use engine::helper::{FillTarget, HelperRead, HelperRole};
// Note HI1: the texture panel's columns, so the helper rows line up with the texture rows.
use super::settings_panel::{column_widths_beside, control_slot, field_label};

const READS: [(Option<HelperRead>, &str); 6] = [
    (None, "Auto (R)"),
    (Some(HelperRead::R), "R"),
    (Some(HelperRead::G), "G"),
    (Some(HelperRead::B), "B"),
    (Some(HelperRead::A), "A"),
    (Some(HelperRead::Luma), "Luminance"),
];

enum Edit {
    Role(Option<HelperRole>),
    Fills(Option<Vec<FillTarget>>),
    Read(Option<HelperRead>),
    Invert(bool),
    Select(String),
}

/// Names `r` (a combo or a button with a short visible text) for a screen reader and tests.
pub(super) fn named(r: &egui::Response, kind: egui::WidgetType, name: &str, value: Option<&str>) {
    r.widget_info(|| {
        let mut i = egui::WidgetInfo::labeled(kind, r.enabled(), name);
        i.current_text_value = value.map(str::to_string);
        i
    });
}

/// Appends `text` in the body colour (`PLACEHOLDER`: the widget's own) or, when `weak`, the weak one.
fn push(job: &mut LayoutJob, style: &egui::Style, text: &str, weak: bool) {
    let color = if weak {
        style.visuals.weak_text_color()
    } else {
        egui::Color32::PLACEHOLDER
    };
    let font_id = egui::TextStyle::Body.resolve(style);
    job.append(text, 0.0, egui::TextFormat::simple(font_id, color));
}

fn push_target(
    job: &mut LayoutJob,
    h: &HelperReport,
    t: FillTarget,
    tails: Tails,
    style: &egui::Style,
) {
    push(job, style, &fill_text(t), false);
    push(job, style, "  ", false);
    push(job, style, &target_file(h, t, tails), true);
}

/// Note HP: a fill as one line: what it writes in the body colour, two spaces, the file it lands in in the weak one.
pub(crate) fn target_job(
    h: &HelperReport,
    t: FillTarget,
    tails: Tails,
    style: &egui::Style,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    push_target(&mut job, h, t, tails, style);
    job
}

/// Note HP: the panel's subtitle: "fills " and the targets joined by ", ", or the weak "not used: <reason>".
pub(crate) fn fills_job(h: &HelperReport, tails: Tails, style: &egui::Style) -> LayoutJob {
    let mut job = LayoutJob::default();
    if h.fills.is_empty() {
        push(
            &mut job,
            style,
            &crate::model::items::helper_target_text(h),
            true,
        );
        return job;
    }
    push(&mut job, style, "fills ", false);
    for (i, t) in h.fills.iter().enumerate() {
        if i > 0 {
            push(&mut job, style, ", ", false);
        }
        push_target(&mut job, h, *t, tails, style);
    }
    job
}

/// The read-only reason on a disabled control's hover.
fn why(r: egui::Response, ro: &Option<String>) -> egui::Response {
    match ro {
        Some(w) => r.on_disabled_hover_text(w),
        None => r,
    }
}

/// A combo in an exact-width (`w`) slot; `f` draws the options. Returns its response.
pub(super) fn combo(
    ui: &mut egui::Ui,
    id: (&str, &str),
    shown: &str,
    w: f32,
    f: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    let h = ui.spacing().interact_size.y;
    ui.allocate_ui(egui::vec2(w, h), |ui| {
        egui::ComboBox::from_id_salt(id)
            .width(w)
            .truncate()
            .selected_text(shown)
            .show_ui(ui, f)
            .response
    })
    .inner
}

pub fn draw(ui: &mut egui::Ui, model: &mut AppModel, key: &str, h: &HelperReport) {
    let Some(item) = model.item(key) else { return };
    let name = item.name.clone();
    // Note HF1: one check from a texture to its set; a no-set texture has none.
    let set_key = model.set_key_of_item(key);
    let ro = model.toml_read_only_reason();
    let mut edits = vec![];
    if let Some(sk) = set_key.filter(|k| !k.is_empty()) {
        let label = super::columns::set_label(&super::columns::export_prefix(model), &sk);
        if ui.link(format!("← {label}")).clicked() {
            model.select_set(&sk);
        }
    }
    ui.heading(&name);
    let tails = model.helper_tails(h);
    ui.label(fills_job(h, tails, ui.style()));
    // Note HH7: every set that shares the file, used or not, each a link to its set.
    if !h.shared_with.is_empty() {
        let prefix = super::columns::export_prefix(model);
        ui.horizontal_wrapped(|ui| {
            ui.label("Shared with");
            for sk in &h.shared_with {
                if ui.link(super::columns::set_label(&prefix, sk)).clicked() {
                    model.select_set(sk);
                }
            }
        });
    }
    ui.add_space(4.0);
    ui.add_enabled_ui(ro.is_none(), |ui| {
        egui::Grid::new(("helper-panel", key))
            .num_columns(3)
            .show(ui, |ui| {
                role_row(ui, key, h, &ro, &mut edits);
                let now = model.current_fills(key, h);
                fills_rows(ui, key, h, tails, now, &ro, &mut edits);
                read_rows(ui, key, h, &ro, &mut edits);
            });
        for (t, reason) in &h.skipped_targets {
            ui.weak(format!("Skipped {}: {reason}", fill_text(*t)));
        }
    });
    for (k, shown) in output_items(ui.ctx(), model, key, h) {
        if ui.button(format!("Show {shown}")).clicked() {
            edits.push(Edit::Select(k));
        }
    }
    for e in edits {
        match e {
            Edit::Role(r) => {
                // A tick left from before the file became a helper must not come back with the panel (Note HI5).
                ui.data_mut(|d| d.remove_temp::<bool>(tick_id(key)));
                model.set_helper_role(key, r);
            }
            Edit::Fills(f) => model.set_helper_fills(key, f),
            Edit::Read(r) => model.set_helper_read(key, r),
            Edit::Invert(on) => model.set_helper_invert(key, on),
            Edit::Select(k) => model.select_item(&k),
        }
    }
}

/// (item key, name) of each output the helper fills: one scan of the items per plan, cached by the items' revision.
fn output_items(
    ctx: &egui::Context,
    model: &AppModel,
    key: &str,
    h: &HelperReport,
) -> Vec<(String, String)> {
    type Slot = (String, u64, Vec<(String, String)>);
    let id = egui::Id::new("helper-panel-outputs");
    let rev = model.items_rev();
    if let Some((k, r, v)) = ctx.data(|d| d.get_temp::<Slot>(id)) {
        if k == key && r == rev {
            return v;
        }
    }
    let v: Vec<(String, String)> = h
        .outputs
        .iter()
        .filter_map(|o| {
            model
                .items
                .iter()
                .find(|i| i.job.as_ref().is_some_and(|j| &j.output == o))
                .map(|i| (i.key.clone(), i.name.clone()))
        })
        .collect();
    ctx.data_mut(|d| d.insert_temp(id, (key.to_string(), rev, v.clone())));
    v
}

/// The helper rows' control width: the texture rows' one, less the Reset (the widest button beside a control) where
/// the row would not fit, so the centre is not widened under the Advanced drawer (Note HI1, rule 3.1).
fn control_w(ui: &egui::Ui) -> f32 {
    let reset = egui::WidgetText::from("Reset")
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Small,
        )
        .size()
        .x;
    let s = ui.spacing();
    column_widths_beside(ui, reset + 2.0 * s.button_padding.x + s.item_spacing.x).1
}

fn hint(ui: &mut egui::Ui, text: &str) {
    ui.add(egui::Label::new(egui::RichText::new(text).small().weak()).truncate());
}

fn reset(ui: &mut egui::Ui, name: &str, hover: &str, ro: &Option<String>) -> bool {
    let r = ui.small_button("Reset");
    named(&r, egui::WidgetType::Button, name, None);
    why(r.on_hover_text(hover), ro).clicked()
}

fn role_row(
    ui: &mut egui::Ui,
    key: &str,
    h: &HelperReport,
    ro: &Option<String>,
    edits: &mut Vec<Edit>,
) {
    field_label(ui, true, "Role");
    let w = control_w(ui);
    ui.horizontal(|ui| {
        if let Some(role) = role_combo(ui, key, Some(h.role), h.role_hand_set, ro, w) {
            edits.push(Edit::Role(Some(role)));
        }
        if h.role_hand_set && reset(ui, "Reset role", "Use the role guessed from the name", ro) {
            edits.push(Edit::Role(None));
        }
    });
    hint(
        ui,
        if h.role_hand_set {
            "set by you"
        } else {
            "guessed from the name"
        },
    );
    ui.end_row();
    // Note HI4, HI16: unticked, the file is an ordinary texture again (`helper.role = "none"`), in every set.
    field_label(ui, true, "Helper");
    let r = control_slot(ui, w, |ui| helper_tick(ui, true, h.shared_with.len(), ro));
    if r.changed() {
        edits.push(Edit::Role(Some(HelperRole::NotHelper)));
    }
    ui.label("");
    ui.end_row();
}

/// The "Helper role" combo (`w` wide): every role but "Not a helper" (the unticked Helper box, Note HI6); `None` shows
/// "Pick a role…". Returns the role picked, if it is to be written: a different role, or the current one while it is
/// not hand-set (an assigned role beats a suggestion).
pub(crate) fn role_combo(
    ui: &mut egui::Ui,
    key: &str,
    current: Option<HelperRole>,
    hand_set: bool,
    ro: &Option<String>,
    w: f32,
) -> Option<HelperRole> {
    let mut picked = None;
    let shown = current.map_or_else(|| "Pick a role…".to_string(), |r| r.label());
    let r = combo(ui, (key, "helper-role"), &shown, w, |ui| {
        for role in HelperRole::all()
            .into_iter()
            .filter(|r| *r != HelperRole::NotHelper)
        {
            let new = Some(role) != current || !hand_set;
            if ui
                .selectable_label(Some(role) == current, role.label())
                .clicked()
                && new
            {
                picked = Some(role);
            }
        }
    });
    named(&r, egui::WidgetType::ComboBox, "Helper role", Some(&shown));
    why(r, ro);
    picked
}

/// The id of a texture's ticked Helper box while it has no role yet (Note HI5): egui temp data, never written.
pub(crate) fn tick_id(key: &str) -> egui::Id {
    egui::Id::new(("helper-tick", key))
}

/// The Helper tick (Note HI4), named "Helper". `shared_with` other sets stop using the file when it is unticked (HI16).
pub(crate) fn helper_tick(
    ui: &mut egui::Ui,
    on: bool,
    shared_with: usize,
    ro: &Option<String>,
) -> egui::Response {
    let mut v = on;
    let r = ui.add_enabled(ro.is_none(), egui::Checkbox::new(&mut v, ""));
    r.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, r.enabled(), v, "Helper")
    });
    let r = match shared_with {
        0 => r,
        1 => r.on_hover_text("Also used by 1 other set: it stops using it too."),
        n => r.on_hover_text(format!(
            "Also used by {n} other sets: they stop using it too."
        )),
    };
    why(r, ro)
}

/// One Grid row per line (`Fills | name ✕ | hint`, then `Add ▾ Reset`), so nothing is centred on a tall cell and the
/// Reset and ✕ share the role row's column. The name is as wide as the combo and inset to its text.
fn fills_rows(
    ui: &mut egui::Ui,
    key: &str,
    h: &HelperReport,
    tails: Tails,
    // Final review M1: the `sdc.toml` list when hand-set (DY18b), so ✕ on one target never drops one that did not land.
    now: Vec<FillTarget>,
    ro: &Option<String>,
    edits: &mut Vec<Edit>,
) {
    let (row_h, pad) = (ui.spacing().interact_size.y, ui.spacing().button_padding.x);
    let w = control_w(ui);
    let hint_text = if h.fills_hand_set {
        "set by you"
    } else {
        "suggested"
    };
    let mut first = true;
    let lead = |ui: &mut egui::Ui, first: &mut bool| {
        field_label(ui, true, if *first { "Fills" } else { "" });
        *first = false;
    };
    for t in &now {
        let was_first = first;
        lead(ui, &mut first);
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(w, row_h),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_size(egui::vec2(w, row_h));
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.add_space(pad);
                    ui.add(egui::Label::new(target_job(h, *t, tails, ui.style())).truncate());
                },
            );
            let hover = format!("Stop filling {}", fill_text(*t));
            let r = ui.small_button("✕");
            named(&r, egui::WidgetType::Button, &hover, None);
            if why(r.on_hover_text(&hover), ro).clicked() {
                // Removing one fill writes the rest as hand-set: the list is the user's from now on.
                let rest = now.iter().copied().filter(|x| x != t).collect();
                edits.push(Edit::Fills(Some(rest)));
            }
        });
        if was_first {
            hint(ui, hint_text);
        }
        ui.end_row();
    }
    let more: Vec<FillTarget> = h
        .options
        .iter()
        .copied()
        .filter(|o| !now.contains(o))
        .collect();
    if more.is_empty() && !h.fills_hand_set && !now.is_empty() {
        return;
    }
    let was_first = first;
    lead(ui, &mut first);
    ui.horizontal(|ui| {
        if more.is_empty() {
            ui.allocate_space(egui::vec2(w, row_h));
        } else {
            let r = combo(ui, (key, "helper-add-fill"), "Add target", w, |ui| {
                for o in &more {
                    if ui
                        .selectable_label(false, target_job(h, *o, tails, ui.style()))
                        .clicked()
                    {
                        let mut next = now.clone();
                        next.push(*o);
                        edits.push(Edit::Fills(Some(next)));
                    }
                }
            });
            named(
                &r,
                egui::WidgetType::ComboBox,
                "Add fill target",
                Some("Add target"),
            );
            why(r, ro);
        }
        if h.fills_hand_set && reset(ui, "Reset fills", "Use the suggested targets", ro) {
            edits.push(Edit::Fills(None));
        }
    });
    if was_first {
        hint(ui, hint_text);
    }
    ui.end_row();
}

fn read_rows(
    ui: &mut egui::Ui,
    key: &str,
    h: &HelperReport,
    ro: &Option<String>,
    edits: &mut Vec<Edit>,
) {
    field_label(ui, true, "Read from");
    let w = control_w(ui);
    let shown = READS.iter().find(|r| r.0 == h.read).map_or("", |r| r.1);
    let r = combo(ui, (key, "helper-read"), shown, w, |ui| {
        for (v, label) in READS {
            if ui.selectable_label(v == h.read, label).clicked() && v != h.read {
                edits.push(Edit::Read(v));
            }
        }
    });
    named(&r, egui::WidgetType::ComboBox, "Read from", Some(shown));
    why(r, ro);
    hint(ui, "the helper's channel used");
    ui.end_row();
    field_label(ui, true, "");
    let mut on = h.invert;
    let r = ui.checkbox(&mut on, "Invert");
    if why(r, ro).changed() {
        edits.push(Edit::Invert(on));
    }
    hint(ui, "white becomes black");
    ui.end_row();
}
