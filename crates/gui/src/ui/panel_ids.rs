//! Every panel of the app is built here. egui 0.37 keys a panel's `PanelState` by `parent_ui.scope_id().with(salt)`,
//! so the same salt has another id in every parent `Ui`; each constructor records the id egui uses, under the salt,
//! for the readers that need the panel's rect or its resize handle (`layout.rs`, the columns' chosen widths, tests).
use eframe::egui::{self, containers::panel::PanelState, Id};

fn record_key(salt: &str) -> Id {
    Id::unique(("sdc-panel-id", salt))
}

fn pending_key(salt: &str) -> Id {
    Id::unique(("sdc-panel-pending", salt))
}

/// Records the id egui will give panel `salt` in `parent`, and gives a pending restored size to the panel (along its
/// axis: x for a left or right panel) before its first build reads its state.
fn record(parent: &egui::Ui, salt: &'static str, along_x: bool) {
    let id = parent.scope_id().with(salt);
    let ctx = parent.ctx();
    ctx.data_mut(|d| d.insert_temp(record_key(salt), id));
    if let Some(size) = ctx.data_mut(|d| d.remove_temp::<f32>(pending_key(salt))) {
        let r = parent.available_rect_before_wrap();
        let size = if along_x {
            egui::vec2(size, r.height())
        } else {
            egui::vec2(r.width(), size)
        };
        let outer_rect = egui::Rect::from_min_size(r.min, size);
        ctx.data_mut(|d| d.insert_persisted(id, PanelState { outer_rect }));
    }
}

/// A left panel named `salt`, shown in `parent`.
pub fn left(parent: &egui::Ui, salt: &'static str) -> egui::Panel {
    record(parent, salt, true);
    egui::Panel::left(salt)
}

/// A right panel named `salt`, shown in `parent`.
pub fn right(parent: &egui::Ui, salt: &'static str) -> egui::Panel {
    record(parent, salt, true);
    egui::Panel::right(salt)
}

/// A top panel named `salt`, shown in `parent`.
pub fn top(parent: &egui::Ui, salt: &'static str) -> egui::Panel {
    record(parent, salt, false);
    egui::Panel::top(salt)
}

/// A bottom panel named `salt`, shown in `parent`.
pub fn bottom(parent: &egui::Ui, salt: &'static str) -> egui::Panel {
    record(parent, salt, false);
    egui::Panel::bottom(salt)
}

/// The id egui keeps panel `salt`'s state under; `None` until the panel was built once.
pub fn panel_id(ctx: &egui::Context, salt: &str) -> Option<Id> {
    ctx.data(|d| d.get_temp(record_key(salt)))
}

/// Panel `salt`'s outer rect as egui stored it (last frame's), if the panel was built.
pub fn panel_rect(ctx: &egui::Context, salt: &str) -> Option<egui::Rect> {
    panel_id(ctx, salt)
        .and_then(|id| PanelState::load(ctx, id))
        .map(|s| s.outer_rect)
}

/// The id of the resize handle of the panel whose id is `panel` (egui 0.37 `panel.rs::resize_widget_id`).
pub fn resize_id(panel: Id) -> Id {
    panel.with("__resize")
}

/// Where the Textures and Sets columns keep the width the user dragged to (`ui::chosen_width_panel`).
pub fn chosen_width_id(salt: &str) -> Id {
    Id::unique(salt).with("chosen")
}

/// A saved size for panel `salt` (its width, or its height for a top or bottom panel), applied when it is next built.
pub fn restore_size(ctx: &egui::Context, salt: &str, size: f32) {
    ctx.data_mut(|d| d.insert_temp(pending_key(salt), size));
}

/// The size [`restore_size`] gave panel `salt` that no build has applied yet.
pub fn pending_size(ctx: &egui::Context, salt: &str) -> Option<f32> {
    ctx.data(|d| d.get_temp(pending_key(salt)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::containers::panel::PanelState;

    fn passes(ctx: &egui::Context, mut f: impl FnMut(&mut egui::Ui)) {
        for _ in 0..3 {
            // No painter takes the font atlas here; epaint 0.37 panics on a dropped, unapplied delta.
            ctx.run_ui(egui::RawInput::default(), &mut f)
                .textures_delta
                .clear();
        }
    }

    #[test]
    fn a_recorded_id_finds_the_panel_state_at_the_root_and_nested() {
        let ctx = egui::Context::default();
        passes(&ctx, |ui| {
            left(ui, "sets").show(ui, |ui| ui.label("Sets"));
            egui::CentralPanel::default().show(ui, |ui| {
                right(ui, "preview-settings")
                    .exact_size(300.0)
                    .show(ui, |ui| ui.label("Settings"));
            });
        });
        assert!(panel_rect(&ctx, "sets").is_some());
        assert_eq!(
            panel_rect(&ctx, "preview-settings").map(|r| r.width()),
            Some(300.0)
        );
        assert!(
            PanelState::load(&ctx, Id::unique("sets")).is_none(),
            "0.37 no longer stores a panel under Id::unique(salt)"
        );
    }

    #[test]
    fn a_restored_size_is_applied_once_when_the_panel_is_first_built() {
        let ctx = egui::Context::default();
        restore_size(&ctx, "meshes-tree", 333.0);
        assert_eq!(pending_size(&ctx, "meshes-tree"), Some(333.0));
        passes(&ctx, |ui| {
            // The content fills the panel: a panel is as wide as what it holds (its minimum aside).
            left(ui, "meshes-tree")
                .default_size(420.0)
                .show(ui, |ui| ui.set_min_width(ui.available_width()));
        });
        assert_eq!(
            panel_rect(&ctx, "meshes-tree").map(|r| r.width()),
            Some(333.0)
        );
        assert_eq!(pending_size(&ctx, "meshes-tree"), None, "applied once");
    }

    #[test]
    fn no_panel_is_built_outside_panel_ids() {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push(p);
                }
            }
        }
        let re = regex::Regex::new(r"\bPanel::(left|right|top|bottom)\(").unwrap();
        let mut files = vec![];
        walk(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        let (mut hits, mut own) = (vec![], false);
        for f in &files {
            let text = std::fs::read_to_string(f).unwrap();
            for (n, line) in text.lines().enumerate() {
                if !re.is_match(line) {
                    continue;
                }
                if f.file_name().is_some_and(|n| n == "panel_ids.rs") {
                    own = true;
                } else {
                    hits.push(format!("{}:{}", f.display(), n + 1));
                }
            }
        }
        assert!(own, "the scan found no panel in panel_ids.rs itself");
        assert!(
            hits.is_empty(),
            "panels built outside ui::panel_ids: {hits:#?}"
        );
    }
}
