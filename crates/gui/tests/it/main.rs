//! Every GUI test file that keeps no process-wide state, built as one test binary (Note HU1): one link and one exe
//! instead of one per file. Each `tests/<name>.rs` is a module here, so a test is named `<name>::<test>`. Run one file
//! with `cargo test -p sdc-gui -p sdc --test it <name>::`.
//!
//! A new test file gets its two lines below. A file that changes process-wide state (an environment variable, the
//! panic hook, the device-lost flag) gets a `[[test]]` entry in `crates/gui/Cargo.toml` instead: here it would
//! change what the tests on the other threads see. `every_test_file_is_built` fails when a file is in neither place
//! or in both.

// Every file declares `mod support;` for itself, as it did as its own binary. A few also declare the fixture builder
// of another crate (`nif`, `esp`, `modscan`) the same way. So those files are loaded as a module more than once.
#![allow(clippy::duplicate_mod)]

#[path = "../advanced.rs"]
mod advanced;
#[path = "../alpha_prop.rs"]
mod alpha_prop;
#[path = "../also_export.rs"]
mod also_export;
#[path = "../audit_gui_data.rs"]
mod audit_gui_data;
#[path = "../auto_accept.rs"]
mod auto_accept;
#[path = "../auto_flags.rs"]
mod auto_flags;
#[path = "../backlog_b4_input_name.rs"]
mod backlog_b4_input_name;
#[path = "../backlog_b4_preview.rs"]
mod backlog_b4_preview;
#[path = "../bc7_weighting.rs"]
mod bc7_weighting;
#[path = "../bottom_panel.rs"]
mod bottom_panel;
#[path = "../bulk_apply.rs"]
mod bulk_apply;
#[path = "../bulk_plan.rs"]
mod bulk_plan;
#[path = "../bulk_selection.rs"]
mod bulk_selection;
#[path = "../bulk_sets.rs"]
mod bulk_sets;
#[path = "../bulk_ui.rs"]
mod bulk_ui;
#[path = "../by_hand_rows.rs"]
mod by_hand_rows;
#[path = "../centre_tabs.rs"]
mod centre_tabs;
#[path = "../checks.rs"]
mod checks;
#[path = "../checks_corpus.rs"]
mod checks_corpus;
#[path = "../cm_json.rs"]
mod cm_json;
#[path = "../coat_card.rs"]
mod coat_card;
#[path = "../color_fields.rs"]
mod color_fields;
#[path = "../column_stats.rs"]
mod column_stats;
#[path = "../column_widths.rs"]
mod column_widths;
#[path = "../complex_grass.rs"]
mod complex_grass;
#[path = "../cs_pbr.rs"]
mod cs_pbr;
#[path = "../cs_pbr_export.rs"]
mod cs_pbr_export;
#[path = "../cs_pbr_ui.rs"]
mod cs_pbr_ui;
#[path = "../cs_skin_json.rs"]
mod cs_skin_json;
#[path = "../cube_preview.rs"]
mod cube_preview;
#[path = "../cubemap_gui.rs"]
mod cubemap_gui;
#[path = "../dds_thumbs.rs"]
mod dds_thumbs;
#[path = "../detail_tails.rs"]
mod detail_tails;
#[path = "../dialog_keys.rs"]
mod dialog_keys;
#[path = "../edges.rs"]
mod edges;
#[path = "../encoding_follows_type.rs"]
mod encoding_follows_type;
#[path = "../export.rs"]
mod export;
#[path = "../export_drawer.rs"]
mod export_drawer;
#[path = "../export_folder.rs"]
mod export_folder;
#[path = "../export_layout.rs"]
mod export_layout;
#[path = "../export_undecided.rs"]
mod export_undecided;
#[path = "../fast_replan.rs"]
mod fast_replan;
#[path = "../fields.rs"]
mod fields;
#[path = "../fill_from_names.rs"]
mod fill_from_names;
#[path = "../generate_removed.rs"]
mod generate_removed;
#[path = "../generated_sidecar.rs"]
mod generated_sidecar;
#[path = "../gui_tweaks.rs"]
mod gui_tweaks;
#[path = "../header_layout.rs"]
mod header_layout;
#[path = "../heatmap_job.rs"]
mod heatmap_job;
#[path = "../heel_height.rs"]
mod heel_height;
#[path = "../helper_panel.rs"]
mod helper_panel;
#[path = "../helper_picker.rs"]
mod helper_picker;
#[path = "../helper_share.rs"]
mod helper_share;
#[path = "../helpers_list.rs"]
mod helpers_list;
#[path = "../history.rs"]
mod history;
#[path = "../input_name_edit.rs"]
mod input_name_edit;
#[path = "../json_errors.rs"]
mod json_errors;
#[path = "../json_pending.rs"]
mod json_pending;
#[path = "../keyboard.rs"]
mod keyboard;
#[path = "../known_paths_txst.rs"]
mod known_paths_txst;
#[path = "../labels.rs"]
mod labels;
#[path = "../landscape_slots.rs"]
mod landscape_slots;
#[path = "../m3b_flow.rs"]
mod m3b_flow;
#[path = "../match_mode_undo.rs"]
mod match_mode_undo;
#[path = "../meshes_skin_preview.rs"]
mod meshes_skin_preview;
#[path = "../meshes_txst_preview.rs"]
mod meshes_txst_preview;
#[path = "../meshes_view.rs"]
mod meshes_view;
#[path = "../messages.rs"]
mod messages;
#[path = "../mo2_button.rs"]
mod mo2_button;
#[path = "../model.rs"]
mod model;
#[path = "../msn_source.rs"]
mod msn_source;
#[path = "../no_codes.rs"]
mod no_codes;
#[path = "../no_rnm_ui.rs"]
mod no_rnm_ui;
#[path = "../no_set.rs"]
mod no_set;
#[path = "../open_cancel.rs"]
mod open_cancel;
#[path = "../open_in_textures.rs"]
mod open_in_textures;
#[path = "../out_size.rs"]
mod out_size;
#[path = "../output_names.rs"]
mod output_names;
#[path = "../overlay.rs"]
mod overlay;
#[path = "../packing_fills.rs"]
mod packing_fills;
#[path = "../parallax.rs"]
mod parallax;
#[path = "../partial_export.rs"]
mod partial_export;
#[path = "../pbr_export.rs"]
mod pbr_export;
#[path = "../pbr_landscape.rs"]
mod pbr_landscape;
#[path = "../pbr_members.rs"]
mod pbr_members;
#[path = "../pgp.rs"]
mod pgp;
#[path = "../pgp_drag.rs"]
mod pgp_drag;
#[path = "../pgp_lock.rs"]
mod pgp_lock;
#[path = "../picker.rs"]
mod picker;
#[path = "../plan_progress.rs"]
mod plan_progress;
#[path = "../popups.rs"]
mod popups;
#[path = "../presets_branches.rs"]
mod presets_branches;
#[path = "../presets_creator.rs"]
mod presets_creator;
#[path = "../presets_window.rs"]
mod presets_window;
#[path = "../preview_model.rs"]
mod preview_model;
#[path = "../preview_readout.rs"]
mod preview_readout;
#[path = "../problems.rs"]
mod problems;
#[path = "../problems_accept.rs"]
mod problems_accept;
#[path = "../problems_rows.rs"]
mod problems_rows;
#[path = "../project.rs"]
mod project;
#[path = "../recovery.rs"]
mod recovery;
#[path = "../reencode_toggle.rs"]
mod reencode_toggle;
#[path = "../rename.rs"]
mod rename;
#[path = "../rename_outputs_scope.rs"]
mod rename_outputs_scope;
#[path = "../rename_scale.rs"]
mod rename_scale;
#[path = "../rename_slots.rs"]
mod rename_slots;
#[path = "../render.rs"]
mod render;
#[path = "../resize_field.rs"]
mod resize_field;
#[path = "../review.rs"]
mod review;
#[path = "../review_json_answer.rs"]
mod review_json_answer;
#[path = "../review_tab.rs"]
mod review_tab;
#[path = "../set_cards.rs"]
mod set_cards;
#[path = "../set_cleanup.rs"]
mod set_cleanup;
#[path = "../set_editor_ui.rs"]
mod set_editor_ui;
#[path = "../set_editor_width.rs"]
mod set_editor_width;
#[path = "../set_groups.rs"]
mod set_groups;
#[path = "../set_groups_ui.rs"]
mod set_groups_ui;
#[path = "../set_order.rs"]
mod set_order;
#[path = "../set_panel.rs"]
mod set_panel;
#[path = "../set_preview_layout.rs"]
mod set_preview_layout;
#[path = "../set_preview_mato.rs"]
mod set_preview_mato;
#[path = "../set_preview_model.rs"]
mod set_preview_model;
#[path = "../set_preview_ui.rs"]
mod set_preview_ui;
#[path = "../set_preview_xpmf.rs"]
mod set_preview_xpmf;
#[path = "../sets_doc.rs"]
mod sets_doc;
#[path = "../sets_filter.rs"]
mod sets_filter;
#[path = "../sets_model.rs"]
mod sets_model;
#[path = "../settings.rs"]
mod settings;
#[path = "../skin_card.rs"]
mod skin_card;
#[path = "../slot_file_drop.rs"]
mod slot_file_drop;
#[path = "../slot_lock.rs"]
mod slot_lock;
#[path = "../slot_menu.rs"]
mod slot_menu;
#[path = "../slot_menu_ui.rs"]
mod slot_menu_ui;
#[path = "../subsurface_mode.rs"]
mod subsurface_mode;
#[path = "../subtype_hints.rs"]
mod subtype_hints;
#[path = "../target_mod.rs"]
mod target_mod;
#[path = "../tessalith.rs"]
mod tessalith;
#[path = "../texture_panel_layout.rs"]
mod texture_panel_layout;
#[path = "../textures_filter.rs"]
mod textures_filter;
#[path = "../truepbr_cards.rs"]
mod truepbr_cards;
#[path = "../txst_map.rs"]
mod txst_map;
#[path = "../type_picker.rs"]
mod type_picker;
#[path = "../ui_smoke.rs"]
mod ui_smoke;
#[path = "../unknown_suffix.rs"]
mod unknown_suffix;
#[path = "../used_by_meshes.rs"]
mod used_by_meshes;
#[path = "../view.rs"]
mod view;
#[path = "../view_cache.rs"]
mod view_cache;
#[path = "../view_menu.rs"]
mod view_menu;
#[path = "../viewport3d.rs"]
mod viewport3d;
#[path = "../vocabulary.rs"]
mod vocabulary;
#[path = "../window_layout.rs"]
mod window_layout;
#[path = "../xpmf_card.rs"]
mod xpmf_card;
#[path = "../xpmf_export.rs"]
mod xpmf_export;
#[path = "../xpmf_json.rs"]
mod xpmf_json;

/// `autotests = false` (Cargo.toml): a file that is neither a module here nor a `[[test]]` target is never compiled,
/// and its tests would pass by not running. A file that is both runs twice, once beside tests it can disturb.
#[test]
fn every_test_file_is_built() {
    // Without the CRs of a Windows checkout, so a whole line can be matched.
    let modules = include_str!("main.rs").replace('\r', "");
    let manifest = include_str!("../../Cargo.toml").replace('\r', "");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let (mut unbuilt, mut twice) = (Vec::new(), Vec::new());
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_stem().unwrap().to_str().unwrap().to_string();
        // A folder with a `main.rs` is a target of its own, as `it` is; the other folders hold shared modules.
        let (file, module) = if path.join("main.rs").is_file() {
            (format!("{name}/main.rs"), false)
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            let lines = format!("\n#[path = \"../{name}.rs\"]\nmod {name};\n");
            (format!("{name}.rs"), modules.contains(&lines))
        } else {
            continue;
        };
        let target = manifest.contains(&format!("\npath = \"tests/{file}\"\n"));
        match (module, target) {
            (false, false) => unbuilt.push(file),
            (true, true) => twice.push(file),
            _ => {}
        }
    }
    unbuilt.sort();
    twice.sort();
    assert!(
        unbuilt.is_empty() && twice.is_empty(),
        "never built (each needs its two lines in tests/it/main.rs, or a [[test]] entry in Cargo.toml): {unbuilt:?}; \
         built twice (keep one of the two): {twice:?}"
    );
}
