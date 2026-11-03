//! The shape card's Texture set row (Note GO): the TXSTs plugins assign to the shape's 3D index, previewed in the
//! 3D view, to check that a record's alternate textures land on the shape they were made for.
use crate::model::meshes::txst_preview::TxstChoice;
use crate::model::meshes::MeshesModel;
use crate::ui::status::AMBER;
use eframe::egui;
use modscan::plugin_store::record_label;
use modscan::store::ShapeView;

const FROM_NIF: &str = "From the NIF";

/// The row for shape `s`, drawn only when a plugin assigns a TXST to its 3D index. Returns the new pick:
/// `Some(None)` = the NIF's own textures, `Some(Some(formid))` = that TXST.
pub fn row(ui: &mut egui::Ui, m: &MeshesModel, s: &ShapeView) -> Option<Option<u32>> {
    let choices = m.txst_choices(s);
    if choices.is_empty() {
        return None;
    }
    let picked = m.txst_pick(s.block);
    let shown = picked
        .and_then(|(f, _)| choices.iter().find(|c| c.formid == f))
        .map_or_else(|| FROM_NIF.to_string(), choice_text);
    let off = if m.edit.is_some() {
        Some("Close Edit to preview texture sets")
    } else if s.shader_kind != 1 {
        Some("Only Lighting shader shapes are previewed")
    } else {
        None
    };
    let mut out = None;
    ui.horizontal(|ui| {
        ui.label("Texture set:");
        // Rule 3.5: the combo's width is a minimum, so it gets an exact slot.
        let size = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
        ui.allocate_ui(size, |ui| {
            let r = ui.add_enabled_ui(off.is_none(), |ui| {
                let r = egui::ComboBox::from_id_salt(("shape-txst", s.block))
                    .selected_text(&shown)
                    .width(size.x)
                    .truncate()
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(picked.is_none(), FROM_NIF).clicked() {
                            out = Some(None);
                        }
                        for c in &choices {
                            let on = picked.is_some_and(|(f, _)| f == c.formid);
                            let r = ui.selectable_label(on, choice_text(c));
                            let r = r.on_hover_ui(|ui| users(ui, m, s, c));
                            if r.clicked() {
                                out = Some(Some(c.formid));
                            }
                        }
                    });
                let name = format!("Texture set of block {}: {shown}", s.block);
                r.response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, &name)
                });
            });
            if let Some(why) = off {
                r.response.on_hover_text(why);
            }
        });
    });
    if let Some((_, slots)) = picked {
        ui.indent(("shape-txst-slots", s.block), |ui| {
            for (i, p) in slots.iter().enumerate().filter(|(_, p)| !p.is_empty()) {
                ui.weak(format!("slot {}: {p}", i + 1));
            }
            if slots.iter().all(String::is_empty) {
                ui.colored_label(AMBER, "The TXST has no textures");
            }
        });
    }
    out
}

/// Which ARMA field a skin texture comes from, by the model slot it re-textures (Note GO4).
pub(super) fn skin_field(model_slot: u32) -> &'static str {
    match model_slot {
        2 | 4 => "male skin texture (NAM0)",
        _ => "female skin texture (NAM1)",
    }
}

fn choice_text(c: &TxstChoice) -> String {
    format!(
        "{} [{:08X}]",
        c.edid.as_deref().unwrap_or("(no EDID)"),
        c.formid
    )
}

/// Who assigns `c`, with the shape name each record stores; a name that is not this shape's is flagged (the game
/// goes by the 3D index, so the record may target another shape).
fn users(ui: &mut egui::Ui, m: &MeshesModel, s: &ShapeView, c: &TxstChoice) {
    let here = s.name.as_deref().unwrap_or("");
    for (i, name) in &c.users {
        let Some(u) = m.plugin_uses.get(*i) else {
            continue;
        };
        let rec = record_label(u.edid.as_deref(), u.formid, &u.sig, &u.plugin);
        let Some(name) = name else {
            ui.label(format!("{rec}, {}", skin_field(u.model_slot)));
            continue;
        };
        ui.label(format!("{rec}, as \"{name}\""));
        if name != here {
            ui.colored_label(
                AMBER,
                format!("The record names \"{name}\", this shape is \"{here}\": the game goes by the 3D index"),
            );
        }
    }
}
