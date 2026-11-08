//! Note DY17: a helper's panel (helper textures spec Goal 6): its role, the channels it fills, the channel it is read
//! from and whether it is inverted. Every value shown is the engine's `HelperReport`; the GUI only writes the keys.
use crate::model::helpers::target_label;
use crate::model::AppModel;
use eframe::egui;
use engine::helper::report::HelperReport;
use engine::helper::{FillTarget, HelperRead, HelperRole};

/// The fixed width of the panel's combos (egui-ui-rules 3.5: `ComboBox::width` is only a minimum).
const COMBO_W: f32 = 200.0;

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

/// The read-only reason on a disabled control's hover.
fn why(r: egui::Response, ro: &Option<String>) -> egui::Response {
    match ro {
        Some(w) => r.on_disabled_hover_text(w),
        None => r,
    }
}

/// A combo in an exact-width slot; `f` draws the options. Returns its response.
fn combo(
    ui: &mut egui::Ui,
    id: (&str, &str),
    shown: &str,
    f: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    let h = ui.spacing().interact_size.y;
    ui.allocate_ui(egui::vec2(COMBO_W, h), |ui| {
        egui::ComboBox::from_id_salt(id)
            .width(COMBO_W)
            .truncate()
            .selected_text(shown)
            .show_ui(ui, f)
            .response
    })
    .inner
}

pub fn draw(ui: &mut egui::Ui, model: &mut AppModel, key: &str, h: &HelperReport) {
    let Some(item) = model.item(key) else { return };
    let (name, set_key) = (item.name.clone(), item.set_key.clone());
    let ro = model.toml_read_only_reason();
    let mut edits = vec![];
    if let Some(sk) = set_key.filter(|k| model.sets.iter().any(|s| &s.key == k)) {
        let label = super::columns::set_label(&super::columns::export_prefix(model), &sk);
        if ui.link(format!("← {label}")).clicked() {
            model.select_set(&sk);
        }
    }
    ui.heading(&name);
    ui.weak(crate::model::items::helper_target_text(h));
    ui.add_space(4.0);
    ui.add_enabled_ui(ro.is_none(), |ui| {
        egui::Grid::new(("helper-panel", key))
            .num_columns(3)
            .show(ui, |ui| {
                role_row(ui, key, h, &ro, &mut edits);
                let now = model.current_fills(key, h);
                fills_rows(ui, key, h, now, &ro, &mut edits);
                read_rows(ui, key, h, &ro, &mut edits);
            });
        for (t, reason) in &h.skipped_targets {
            ui.weak(format!("Skipped {}: {reason}", target_label(h, *t)));
        }
    });
    for (k, shown) in output_items(ui.ctx(), model, key, h) {
        if ui.button(format!("Show {shown}")).clicked() {
            edits.push(Edit::Select(k));
        }
    }
    for e in edits {
        match e {
            Edit::Role(r) => model.set_helper_role(key, r),
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
    ui.label("Role");
    ui.horizontal(|ui| {
        if let Some(role) = role_combo(ui, key, h.role, h.role_hand_set, ro) {
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
}

/// The "Helper role" combo (fixed width): "Not a helper" then every role. Returns the role picked, if it is to be
/// written: a different role, or the current one while it is not hand-set (an assigned role beats a suggestion).
pub(crate) fn role_combo(
    ui: &mut egui::Ui,
    key: &str,
    current: HelperRole,
    hand_set: bool,
    ro: &Option<String>,
) -> Option<HelperRole> {
    let mut picked = None;
    let shown = current.label();
    let r = combo(ui, (key, "helper-role"), &shown, |ui| {
        // "Not a helper" first (`all()`'s order), so it is in view without scrolling the long list.
        for role in HelperRole::all() {
            let new = role != current || !hand_set;
            if ui.selectable_label(role == current, role.label()).clicked() && new {
                picked = Some(role);
            }
            if role == HelperRole::NotHelper {
                ui.separator();
            }
        }
    });
    named(&r, egui::WidgetType::ComboBox, "Helper role", Some(&shown));
    why(r, ro);
    picked
}

/// One Grid row per line (`Fills | name ✕ | hint`, then `Add ▾ Reset`), so nothing is centred on a tall cell and the
/// Reset and ✕ share the role row's column. The name is as wide as the combo and inset to its text.
fn fills_rows(
    ui: &mut egui::Ui,
    key: &str,
    h: &HelperReport,
    // Final review M1: the `sdc.toml` list when hand-set (DY18b), so ✕ on one target never drops one that did not land.
    now: Vec<FillTarget>,
    ro: &Option<String>,
    edits: &mut Vec<Edit>,
) {
    let (row_h, pad) = (ui.spacing().interact_size.y, ui.spacing().button_padding.x);
    let hint_text = if h.fills_hand_set {
        "set by you"
    } else {
        "suggested"
    };
    let mut first = true;
    let lead = |ui: &mut egui::Ui, first: &mut bool| {
        ui.label(if *first { "Fills" } else { "" });
        *first = false;
    };
    for t in &now {
        let label = target_label(h, *t);
        let was_first = first;
        lead(ui, &mut first);
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(COMBO_W, row_h),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_size(egui::vec2(COMBO_W, row_h));
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.add_space(pad);
                    ui.add(egui::Label::new(&label).truncate());
                },
            );
            let hover = format!("Stop filling {label}");
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
            ui.allocate_space(egui::vec2(COMBO_W, row_h));
        } else {
            let r = combo(ui, (key, "helper-add-fill"), "Add target", |ui| {
                for o in &more {
                    if ui.selectable_label(false, target_label(h, *o)).clicked() {
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
    ui.label("Read from");
    let shown = READS.iter().find(|r| r.0 == h.read).map_or("", |r| r.1);
    let r = combo(ui, (key, "helper-read"), shown, |ui| {
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
    ui.label("");
    let mut on = h.invert;
    let r = ui.checkbox(&mut on, "Invert");
    if why(r, ro).changed() {
        edits.push(Edit::Invert(on));
    }
    hint(ui, "white becomes black");
    ui.end_row();
}
