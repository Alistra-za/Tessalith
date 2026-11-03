//! The Advanced Skin card (Note GN) under a FaceGen (4) or FaceGen RGB tint (5) shape's card: RFAOS and wetness map
//! picks (Auto / a mod list or project texture) and a wet amount slider, for the 3D preview only.
use crate::model::known_paths::filter;
use crate::model::meshes::skin_preview::{SkinMap, SkinPick, PROJECT_KEY};
use crate::model::meshes::MeshesModel;
use crate::ui::path_picker;
use eframe::egui;
use modscan::store::ShapeView;

/// What the card asks for this frame.
pub enum SkinAct {
    Pick(SkinMap, Option<SkinPick>),
    Wet(f32),
}

/// Mod list textures listed at most per filter.
const MOD_LIMIT: usize = 200;
/// How long the project folder's DDS list is reused before it is read again.
const PROJECT_TTL_SECS: f64 = 10.0;

/// The card body for shape `s` (the caller draws the frame). Returns at most one action.
pub fn body(ui: &mut egui::Ui, m: &MeshesModel, s: &ShapeView) -> Option<SkinAct> {
    let mut act = None;
    let r = ui.label(egui::RichText::new("Advanced Skin preview").strong());
    r.on_hover_text(
        "Community Shaders' extra skin textures, for this preview only: nothing is written to the NIF, the \
         project or the game. Resets when another mesh is shown.",
    );
    let e = m.skin_entry(s.block);
    let p = e.map(|e| &e.preview);
    for (map, name, picked) in [
        (SkinMap::Rfaos, "RFAOS", p.and_then(|p| p.rfaos.as_deref())),
        (
            SkinMap::Wetness,
            "Wetness map",
            p.and_then(|p| p.wet_map.as_deref()),
        ),
    ] {
        if let Some(a) = row(ui, m, s, map, name, picked) {
            act = Some(a);
        }
    }
    let mut wet = p.map_or(0.0, |p| p.wet);
    let r = ui.add(egui::Slider::new(&mut wet, 0.0..=1.0).text("Wet amount"));
    r.widget_info(|| {
        egui::WidgetInfo::slider(
            true,
            f64::from(wet),
            format!("Wet amount of block {}", s.block),
        )
    });
    let r = r.on_hover_text(
        "Community Shaders' skin wetness: 0 dry, 1 soaked. It stands in for rain or sweat in game.",
    );
    if r.changed() {
        act = Some(SkinAct::Wet(wet));
    }
    act
}

/// One map's row: its name and what the preview uses, then Browse and (when picked) × back to Auto.
fn row(
    ui: &mut egui::Ui,
    m: &MeshesModel,
    s: &ShapeView,
    map: SkinMap,
    name: &str,
    picked: Option<&str>,
) -> Option<SkinAct> {
    let mut act = None;
    let shown = match picked {
        Some(k) => k.strip_prefix(PROJECT_KEY).unwrap_or(k).to_string(),
        None => "Auto".to_string(),
    };
    let block = s.block;
    egui::Sides::new().shrink_left().truncate().show(
        ui,
        |ui| {
            let r = ui.label(format!("{name}: {shown}"));
            // Rule 8.4: every skin card has these rows; the block tells them apart.
            let label = format!("{name} of block {block}: {shown}");
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &label));
            r.on_hover_ui(|ui| {
                ui.label(hover(m, s, map, picked));
            });
        },
        |ui| {
            if picked.is_some() {
                let r = ui.small_button("×").on_hover_text("Back to Auto");
                let label = format!("{name} of block {block}: back to Auto");
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
                if r.clicked() {
                    act = Some(SkinAct::Pick(map, None));
                }
            }
            let b = path_picker::menu(ui, "Browse", |ui| browse(ui, m, (block, map)));
            let label = format!("Browse for the {name} of block {block}");
            b.response
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
            if let Some(Some(p)) = b.inner {
                act = Some(SkinAct::Pick(map, Some(p)));
            }
        },
    );
    act
}

/// The full path in use and where it came from (built only while hovered).
fn hover(m: &MeshesModel, s: &ShapeView, map: SkinMap, picked: Option<&str>) -> String {
    if let Some(k) = picked {
        return match k.strip_prefix(PROJECT_KEY) {
            Some(rel) => format!("Picked from the project: {rel}"),
            None => format!("Picked from the mod list: {k}"),
        };
    }
    let slot = |i: usize| s.textures.get(i).cloned().flatten().unwrap_or_default();
    let ty = s.shader_type.unwrap_or(0);
    let suffix = match map {
        SkinMap::Rfaos => "_rfaos",
        SkinMap::Wetness => "_wet",
    };
    match m.skin_auto(&slot(1), &slot(7), ty, map) {
        Some(p) if map == SkinMap::Wetness => {
            format!("Auto: {p}\n(used while the wet amount is above 0)")
        }
        Some(p) => format!("Auto: {p}"),
        None => format!(
            "Auto: no {suffix} texture next to the _s or _n map in the mod list, so none: \
             Community Shaders' default"
        ),
    }
}

/// The Browse popup: one filter over the mod list's scanned textures and the project's exported DDS files.
fn browse(ui: &mut egui::Ui, m: &MeshesModel, salt: (u32, SkinMap)) -> Option<SkinPick> {
    let fid = egui::Id::new(("skin-card-browse-filter", salt));
    let mut q: String = ui.data(|d| d.get_temp(fid)).unwrap_or_default();
    ui.add(
        egui::TextEdit::singleline(&mut q)
            .hint_text("Filter textures")
            .desired_width(360.0),
    );
    ui.data_mut(|d| d.insert_temp(fid, q.clone()));
    let mods = path_picker::cached(ui, "skin-card-mods", (0, hash(&q)), || {
        m.texture_paths(&q, MOD_LIMIT)
    });
    let bucket = (ui.input(|i| i.time) / PROJECT_TTL_SECS) as usize;
    let root = m.nif_export_root.as_deref().map(|p| p.to_string_lossy());
    let project = path_picker::cached(ui, "skin-card-project", (bucket, hash(&root)), || {
        m.project_textures()
    });
    let project_hits = filter(&project, &q, path_picker::LIMIT);
    let mut picked = None;
    egui::ScrollArea::vertical()
        .id_salt(("skin-card-browse-scroll", salt))
        .max_height(320.0)
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Mod list").strong());
            if mods.is_empty() {
                ui.weak("No scanned texture matches.");
            }
            for p in mods.iter() {
                if ui.selectable_label(false, p.as_str()).clicked() {
                    picked = Some(SkinPick::Mod(p.clone()));
                }
            }
            ui.separator();
            ui.label(egui::RichText::new("Project (Textures export folder)").strong());
            if root.is_none() {
                ui.weak("No Textures export folder is set.");
            } else if project_hits.is_empty() {
                ui.weak("No exported DDS matches.");
            }
            for p in project_hits {
                if ui.selectable_label(false, p).clicked() {
                    picked = Some(SkinPick::Project(p.to_string()));
                }
            }
        });
    if picked.is_some() {
        ui.data_mut(|d| d.remove::<String>(fid));
        ui.close();
    }
    picked
}

fn hash(v: &impl std::hash::Hash) -> u64 {
    use std::hash::{BuildHasher, BuildHasherDefault};
    BuildHasherDefault::<std::collections::hash_map::DefaultHasher>::default().hash_one(v)
}
