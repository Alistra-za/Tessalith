//! Every GUI test file that keeps no process-wide state, built as one test binary (Note HU1): one link and one exe
//! instead of one per file. Each `tests/<name>.rs` is a module here, so a test is named `<name>::<test>`. Run one file
//! with `cargo test -p sdc-gui -p sdc --test it <name>::`.
//!
//! A new test file gets its two lines below. A file that changes process-wide state (an environment variable, the
//! panic hook, the device-lost flag) gets a `[[test]]` entry in `crates/gui/Cargo.toml` instead: here it would
//! change what the tests on the other threads see. `every_test_file_is_built` fails when a file is in neither place
//! or in both.

#![allow(
    clippy::duplicate_mod,
    reason = "each file declares mod support (and a few the nif, esp or modscan fixture builder) for itself, as it did as its own binary"
)]

#[path = "../accepted_037.rs"]
mod accepted_037;
#[path = "../accessible_names.rs"]
mod accessible_names;
#[path = "../ack_offer.rs"]
mod ack_offer;
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
#[path = "../effort_search.rs"]
mod effort_search;
#[path = "../egui_gotchas.rs"]
mod egui_gotchas;
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
#[path = "../export_shell.rs"]
mod export_shell;
#[path = "../export_undecided.rs"]
mod export_undecided;
#[path = "../fast_replan.rs"]
mod fast_replan;
#[path = "../fields.rs"]
mod fields;
#[path = "../fill_from_names.rs"]
mod fill_from_names;
#[path = "../fuzz_follow.rs"]
mod fuzz_follow;
#[path = "../generate_removed.rs"]
mod generate_removed;
#[path = "../generated_sidecar.rs"]
mod generated_sidecar;
#[path = "../grey_format.rs"]
mod grey_format;
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
#[path = "../inspector_section.rs"]
mod inspector_section;
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
#[path = "../look_036.rs"]
mod look_036;
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
#[path = "../normal_alpha.rs"]
mod normal_alpha;
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
#[path = "../problems_notes.rs"]
mod problems_notes;
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
#[path = "../scroll_bars.rs"]
mod scroll_bars;
#[path = "../set_cards.rs"]
mod set_cards;
#[path = "../set_cleanup.rs"]
mod set_cleanup;
#[path = "../set_drag.rs"]
mod set_drag;
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
#[path = "../settings_tab.rs"]
mod settings_tab;
#[path = "../shown_text.rs"]
mod shown_text;
#[path = "../skin_card.rs"]
mod skin_card;
#[path = "../slot_alpha.rs"]
mod slot_alpha;
#[path = "../slot_file_drop.rs"]
mod slot_file_drop;
#[path = "../slot_links.rs"]
mod slot_links;
#[path = "../slot_lock.rs"]
mod slot_lock;
#[path = "../slot_menu.rs"]
mod slot_menu;
#[path = "../slot_menu_ui.rs"]
mod slot_menu_ui;
#[path = "../start_shell.rs"]
mod start_shell;
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
#[path = "../wet_assign.rs"]
mod wet_assign;
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

/// The process-state needles, built here so this file is not a hit. The first four and the last (the global allocator
/// attribute) are std's; the fifth and sixth are the GUI's own wrappers (`crash_log::install_in` takes and sets the
/// panic hook, `crash_log::watch_device` sets the device-lost flag), which the std needles cannot see from a test file.
fn process_state_needles() -> Vec<String> {
    vec![
        format!("{}_var(", "set"),
        format!("{}_var(", "remove"),
        format!("panic::{}_hook", "set"),
        format!("{}_hook", "take"),
        format!("crash_log::{}", "install"),
        format!("{}_device(", "watch"),
        format!("{}_allocator", "global"),
    ]
}

/// True when `line` holds a needle that does not continue an identifier: the byte before it is not a letter, a digit
/// or `_`, so `offset_var(` and `unset_var(` are not `set_var(`.
fn holds_needle(line: &str, needles: &[String]) -> bool {
    needles.iter().any(|s| {
        line.match_indices(s.as_str()).any(|(at, _)| {
            !line[..at]
                .bytes()
                .next_back()
                .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_')
        })
    })
}

/// Reads `root` and every `mod x;` file under it: a `#[path]` on the line above, else `x/mod.rs` or `x.rs` beside
/// the declaring file. A declaration it cannot resolve goes to `unresolved` as `file:line`. Files already in `seen`
/// are skipped.
// shortcut: line by line, so a `mod` inside a block, a macro or a `cfg_attr(path)` is not followed, and an inline
// `mod x { .. }` is read as part of its file; upgrade when a test file declares modules that way.
fn read_modules(
    root: &std::path::Path,
    seen: &mut std::collections::HashSet<std::path::PathBuf>,
    unresolved: &mut Vec<String>,
) -> Vec<(std::path::PathBuf, String)> {
    let (mut out, mut todo) = (Vec::new(), vec![root.to_path_buf()]);
    while let Some(p) = todo.pop() {
        if !seen.insert(p.clone()) {
            continue;
        }
        let text = std::fs::read_to_string(&p).unwrap();
        let base = p.parent().unwrap().to_path_buf();
        let mut prev = "";
        for (n, l) in text.lines().enumerate() {
            let t = l.trim();
            let t = match t.strip_prefix("pub") {
                Some(r) if r.starts_with(' ') => r.trim_start(),
                Some(r) if r.starts_with('(') => r[r.find(')').unwrap() + 1..].trim_start(),
                _ => t,
            };
            if let Some(name) = t.strip_prefix("mod ").and_then(|r| r.strip_suffix(';')) {
                let by_attr = prev
                    .strip_prefix("#[path = \"")
                    .and_then(|r| r.strip_suffix("\"]"))
                    .map(|r| base.join(r));
                let mut found = by_attr.into_iter().chain([
                    base.join(name).join("mod.rs"),
                    base.join(format!("{name}.rs")),
                ]);
                match found.find(|f| f.is_file()) {
                    Some(f) => todo.push(f),
                    None => unresolved.push(format!("{}:{}: mod {name};", p.display(), n + 1)),
                }
            }
            prev = l.trim();
        }
        out.push((p, text));
    }
    out
}

/// A module of `it` shares one process with the rest, so it must not change process-wide state.
#[test]
fn no_it_module_changes_process_state() {
    let needles = process_state_needles();
    let main = include_str!("main.rs").replace('\r', "");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let (mut hits, mut unresolved) = (Vec::new(), Vec::new());
    let mut seen = std::collections::HashSet::new();
    for (at, _) in main.match_indices("#[path = \"../") {
        let rest = &main[at + "#[path = \"../".len()..];
        let file = &rest[..rest.find('"').unwrap()];
        for (p, text) in read_modules(&dir.join(file), &mut seen, &mut unresolved) {
            for (n, l) in text.lines().enumerate() {
                if holds_needle(l, &needles) {
                    hits.push(format!("{}:{}", p.display(), n + 1));
                }
            }
        }
    }
    let scanned = seen.len();
    assert!(scanned > 100, "the scan read only {scanned} files");
    assert!(
        unresolved.is_empty(),
        "a `mod` the process-state scan cannot find a file for; point it at the file with #[path] or extend read_modules: {unresolved:#?}"
    );
    assert!(
        hits.is_empty(),
        "{scanned} files read; a module of the `it` binary changes process-wide state; give the file a [[test]] entry in crates/gui/Cargo.toml instead of a module line in tests/it/main.rs, or, if the hit is in a shared module, move the call into the [[test]] file that needs it: {hits:#?}"
    );
}

/// The needle list is pinned against the real corpus: every `[[test]]` kept apart for process-wide state holds at
/// least one needle, and the list is the manifest's (Note HU2).
#[test]
fn process_state_needles_match_the_files_kept_apart() {
    // Kept apart for process-wide state: the eight that redirect %LOCALAPPDATA% / %APPDATA% (directly, or through
    // `app_harness`; the last of them is `alloc_frame`, which also holds a global allocator), the two that install the
    // panic hook, `device_lost` (the device-lost flag) and `fonts_env`.
    // `it` is the shared binary and `ui_drive` is a driver with its own `main` (`harness = false`), not kept apart
    // for state, so neither is on the list.
    let apart = [
        "app_no_local_store",
        "app_shell",
        "doc_shots",
        "meshes_header",
        "nif_edit",
        "replan_timing",
        "ui_proof",
        "crash_recovery",
        "fonts_crash_hook",
        "device_lost",
        "fonts_env",
        "alloc_frame",
    ];
    let manifest = include_str!("../../Cargo.toml").replace('\r', "");
    let mut targets: Vec<&str> = manifest
        .lines()
        .filter_map(|l| l.strip_prefix("path = \"tests/")?.strip_suffix(".rs\""))
        .collect();
    targets.sort();
    let mut expected: Vec<&str> = apart.to_vec();
    expected.push("it/main");
    expected.push("ui_drive/main");
    expected.sort();
    assert_eq!(
        targets, expected,
        "a [[test]] in crates/gui/Cargo.toml is not on this list (add it if it changes process-wide state, or say \
         in the comment above `apart` why it is separate), or a listed file lost its entry"
    );
    let needles = process_state_needles();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let (mut bare, mut unresolved) = (Vec::new(), Vec::new());
    for name in apart {
        let files = read_modules(
            &dir.join(format!("{name}.rs")),
            &mut Default::default(),
            &mut unresolved,
        );
        // A needle in a comment (a doc line naming the wrapper) does not count: the real call must be there.
        let holds = files.iter().any(|(_, t)| {
            t.lines()
                .any(|l| !l.trim_start().starts_with("//") && holds_needle(l, &needles))
        });
        if !holds {
            bare.push(name);
        }
    }
    assert!(unresolved.is_empty(), "unresolved modules: {unresolved:#?}");
    assert!(
        bare.is_empty(),
        "kept apart for process-wide state but holding no needle, so the scan cannot see what it changes; find the state it changes and add a needle for it in process_state_needles: {bare:?}"
    );
}

/// Pushes every `.rs` file under `root`, recursively. A `root` that does not exist is skipped unless `required`; every
/// other read error (listing a directory, reading an entry, reading an entry's file type) propagates.
fn rs_files(
    root: &std::path::Path,
    required: bool,
    out: &mut Vec<std::path::PathBuf>,
) -> std::io::Result<()> {
    match std::fs::metadata(root) {
        Err(e) if !required && e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
        Ok(_) => {}
    }
    for e in std::fs::read_dir(root)? {
        let e = e?;
        let p = e.path();
        if e.file_type()?.is_dir() {
            rs_files(&p, true, out)?;
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
    Ok(())
}

/// `line` with the bodies of string and char literals and any `//` comment removed, so a brace or a keyword in a
/// literal does not count. A string or char keeps its quotes.
fn strip_literals(line: &str) -> String {
    let c: Vec<char> = line.chars().collect();
    let (mut out, mut i) = (String::new(), 0);
    while i < c.len() {
        let ident_before = i > 0 && (c[i - 1].is_alphanumeric() || c[i - 1] == '_');
        if c[i] == '/' && c.get(i + 1) == Some(&'/') {
            break;
        }
        if c[i] == 'r' && !ident_before {
            let mut j = i + 1;
            while c.get(j) == Some(&'#') {
                j += 1;
            }
            if c.get(j) == Some(&'"') {
                let hashes = j - i - 1;
                let close = (j + 1..c.len())
                    .find(|&k| c[k] == '"' && (1..=hashes).all(|h| c.get(k + h) == Some(&'#')));
                out.push_str("\"\"");
                let Some(k) = close else { break };
                i = k + 1 + hashes;
                continue;
            }
        }
        if c[i] == '"' {
            i += 1;
            while i < c.len() && c[i] != '"' {
                i += if c[i] == '\\' { 2 } else { 1 };
            }
            i += 1;
            out.push_str("\"\"");
        } else if c[i] == '\'' && c.get(i + 1) == Some(&'\\') {
            i += 3;
            while i < c.len() && c[i] != '\'' {
                i += 1;
            }
            i += 1;
            out.push_str("''");
        } else if c[i] == '\'' && c.get(i + 2) == Some(&'\'') {
            i += 3;
            out.push_str("''");
        } else {
            out.push(c[i]);
            i += 1;
        }
    }
    out
}

/// One source line: its code (see `strip_literals`) and the brace depth at its start, its lowest depth inside it and
/// its depth at its end.
struct CodeLine {
    code: String,
    start: i32,
    low: i32,
    end: i32,
}

fn code_lines(text: &str) -> Vec<CodeLine> {
    let mut depth = 0;
    text.lines()
        .map(|l| {
            let code = strip_literals(l);
            let (start, mut low) = (depth, depth);
            for ch in code.chars() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        low = low.min(depth);
                    }
                    _ => {}
                }
            }
            CodeLine {
                code,
                start,
                low,
                end: depth,
            }
        })
        .collect()
}

fn is_fn_line(line: &str) -> bool {
    let mut t = line.trim_start();
    if let Some(r) = t.strip_prefix("pub") {
        t = match r.strip_prefix('(') {
            Some(inner) => inner.split_once(')').map_or(r, |(_, after)| after),
            None => r,
        }
        .trim_start();
    }
    while let Some(r) = ["const ", "async ", "unsafe "]
        .iter()
        .find_map(|q| t.strip_prefix(q))
    {
        t = r.trim_start();
    }
    t.starts_with("fn ")
}

/// True when `code` calls `helper` as itself: not as the tail of a longer identifier, and not under a `!`, where the
/// helper's printed reason belongs to the other branch.
fn calls_helper(code: &str, helper: &str) -> bool {
    code.match_indices(helper).any(|(at, _)| {
        let before = &code[..at];
        let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
        let path = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == ':';
        !before.bytes().next_back().is_some_and(ident)
            && !before.trim_end_matches(path).trim_end().ends_with('!')
    })
}

/// True when `code` holds `return;`, `return }` or `return Ok(());`.
fn has_return(code: &str) -> bool {
    code.match_indices("return").any(|(at, _)| {
        let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
        let before = code[..at].bytes().next_back().is_some_and(ident);
        let rest = &code[at + 6..];
        let after = rest.bytes().next().is_some_and(ident);
        let rest = rest.trim_start();
        !before
            && !after
            && (rest.starts_with(';') || rest.starts_with('}') || rest.starts_with("Ok(())"))
    })
}

/// Skip helpers that print their own reason before a test returns (Note LH3).
const SKIP_HELPERS: [&str; 2] = ["render_state()", "skip_byte_pins()"];

/// The early returns in the `#[test]` functions of `text` that give no reason (Note LH3), as `label:line`, and the
/// `helpers` entries seen at a return line.
// shortcut: line by line, so a string (raw or ordinary) spanning lines and a `/* */` block comment can miscount braces,
// a `return` inside a closure counts as the test's own, and a match arm `=> return,` is not seen as a return at all;
// upgrade when a test does any of these.
// shortcut: the checks are positional, so a print whose block closes on its own line before the return
// (`if a() { println!("a"); }` above `if b() { return; }`) and a helper behind `||` on the condition line
// (`if other() || skip_byte_pins() {`) both pass; upgrade to tracking the block at the print token and matching the
// exact guard shapes (`if helper() {`, `let ... = helper() else { return };`) when a test writes either shape.
fn unexplained_returns(
    label: &str,
    text: &str,
    helpers: &[&str],
) -> (Vec<String>, std::collections::BTreeSet<String>) {
    const PRINTS: [&str; 4] = ["eprintln!(", "println!(", "eprint!(", "print!("];
    let ends = |l: &CodeLine, set: &[char]| {
        l.code
            .trim_end()
            .chars()
            .last()
            .is_some_and(|c| set.contains(&c))
    };
    let raw: Vec<&str> = text.lines().collect();
    let ln = code_lines(text);
    let (mut hits, mut seen) = (Vec::new(), std::collections::BTreeSet::new());
    let mut i = 0;
    while i < raw.len() {
        if !is_fn_line(raw[i]) {
            i += 1;
            continue;
        }
        // The attribute block: the run of lines above the `fn`, up to a blank line or a statement or block end.
        let mut a = i;
        while a > 0 && !raw[a - 1].trim().is_empty() && !ends(&ln[a - 1], &[';', '{', '}']) {
            a -= 1;
        }
        let attrs = &raw[a..i];
        if !attrs.iter().any(|l| l.trim() == "#[test]") {
            i += 1;
            continue;
        }
        let ignored = attrs.iter().any(|l| l.trim().starts_with("#[ignore = \""));
        let d0 = ln[i].start;
        let (mut opened, mut nested) = (false, None::<(i32, bool)>);
        let mut j = i;
        while j < raw.len() {
            let l = &ln[j];
            let skip = if let Some((nd, op)) = nested.as_mut() {
                *op |= l.code.contains('{');
                if *op && l.end <= *nd {
                    nested = None;
                }
                true
            } else if j > i && is_fn_line(raw[j]) {
                let op = l.code.contains('{');
                if !(op && l.end <= l.start) {
                    nested = Some((l.start, op));
                }
                true
            } else {
                false
            };
            if !skip && has_return(&l.code) {
                let mut excused = ignored;
                for h in helpers {
                    let above =
                        j > i && ends(&ln[j - 1], &['{']) && calls_helper(&ln[j - 1].code, h);
                    if calls_helper(&l.code, h) || above {
                        seen.insert((*h).to_owned());
                        excused = true;
                    }
                }
                for p in j.saturating_sub(3).max(i)..=j {
                    if PRINTS.iter().any(|m| ln[p].code.contains(m))
                        && ln[p + 1..=j].iter().all(|x| x.low >= ln[p].end)
                    {
                        excused = true;
                    }
                }
                if j > i && ends(&ln[j - 1], &[';']) {
                    let mut k = j - 1;
                    while k > i && !ends(&ln[k - 1], &[';', '{', '}']) {
                        k -= 1;
                    }
                    excused |= raw[k].trim_start().starts_with("assert");
                }
                if !excused {
                    hits.push(format!("{label}:{}", j + 1));
                }
            }
            opened |= l.code.contains('{');
            if opened && l.end <= d0 {
                break;
            }
            j += 1;
        }
        i = j + 1;
    }
    (hits, seen)
}

/// kittest's accessibility and glyph checks stay on in every test file and in the GUI's own source.
#[test]
fn kittest_checks_are_never_switched_off() {
    // The needles are built here, so this file is not a hit.
    // shortcut: line by line over `crates/gui/tests` and `crates/gui/src`, so a call split across lines or reached
    // through a helper is not seen; upgrade when a harness builder is wrapped.
    let needles = [
        format!("with_accessibility_{}(", "check"),
        format!("allow_missing_{}(", "glyphs"),
    ];
    let mut files = Vec::new();
    for sub in ["tests", "src"] {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(sub);
        rs_files(&root, true, &mut files).unwrap_or_else(|e| panic!("{}: {e}", root.display()));
    }
    let mut hits = Vec::new();
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap();
        for (n, l) in text.lines().enumerate() {
            if needles.iter().any(|s| l.contains(s.as_str())) {
                hits.push(format!("{}:{}", f.display(), n + 1));
            }
        }
    }
    assert!(
        files.len() > 300,
        "the scan read only {} files",
        files.len()
    );
    assert!(
        hits.is_empty(),
        "{} files read; a test or source file switches off a kittest check; remove the call and fix what the check reports instead: {hits:#?}",
        files.len()
    );
}

/// A test that returns early prints why, is `#[ignore = "..."]`, ends a checked exit, or goes through a skip helper
/// (Note LH3).
#[test]
fn every_early_return_in_a_test_says_why() {
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for krate in ["engine", "texcore", "gui", "cli", "bcenc", "encoders-sys"] {
        for (sub, required) in [("src", true), ("tests", false)] {
            let root = crates.join(krate).join(sub);
            rs_files(&root, required, &mut files)
                .unwrap_or_else(|e| panic!("{}: {e}", root.display()));
        }
    }
    assert!(
        files.len() > 500,
        "the scan read only {} files",
        files.len()
    );
    let (mut hits, mut seen) = (Vec::new(), std::collections::BTreeSet::new());
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let label = f.strip_prefix(&crates).unwrap_or(f).display().to_string();
        let (h, s) = unexplained_returns(&label, &text, &SKIP_HELPERS);
        hits.extend(h);
        seen.extend(s);
    }
    for h in SKIP_HELPERS {
        assert!(
            seen.contains(h),
            "{h} is a skip helper no return uses; drop it from SKIP_HELPERS"
        );
    }
    assert!(
        hits.is_empty(),
        "{} files read; a test returns early without saying why: print the reason (eprintln!(\"skipped: ...\")) on or within three lines above the return, or give the test #[ignore = \"...\"], or end it after an assertion: {hits:#?}",
        files.len()
    );
}

/// The matcher tells an unexplained return from a skip or a checked exit. Each case is one source line, so the scans
/// that read this file do not see a fixture as a test.
#[test]
fn the_return_rule_tells_a_skip_from_a_checked_exit() {
    let hits = |src: &str| unexplained_returns("x.rs", src, &SKIP_HELPERS);
    // 1: a bare return is a hit; 2: a print above it explains it.
    assert_eq!(
        hits("#[test]\nfn t() {\n    if !cfg!(windows) {\n        return;\n    }\n}").0,
        ["x.rs:4"]
    );
    assert!(hits("#[test]\nfn t() {\n    if !cfg!(windows) {\n        eprintln!(\"skipped: Windows only\");\n        return;\n    }\n}").0.is_empty());
    // 3: #[ignore = "..."] explains it, a bare #[ignore] does not.
    assert!(hits("#[test]\n#[ignore = \"needs a GPU\"]\nfn t() {\n    let Ok(o) = std::env::var(\"X\") else {\n        return;\n    };\n}").0.is_empty());
    assert_eq!(hits("#[test]\n#[ignore]\nfn t() {\n    let Ok(o) = std::env::var(\"X\") else {\n        return;\n    };\n}").0, ["x.rs:5"]);
    // 4: a return after an assertion (rule (d)) is a checked exit.
    assert!(hits("#[test]\nfn t() {\n    for _ in 0..3 {\n        if let Some(v) = f() {\n            assert_eq!(\n                v,\n                1\n            );\n            return;\n        }\n    }\n    panic!();\n}").0.is_empty());
    // 5: a skip helper on the return line, and it is reported as seen.
    let (h, seen) =
        hits("#[test]\nfn t() {\n    let Some(rs) = render_state() else { return };\n}");
    assert!(
        h.is_empty() && seen.contains("render_state()"),
        "{h:?} {seen:?}"
    );
    // 6: the render.rs shape: the helper excuses its own line only.
    assert_eq!(hits("#[test]\nfn t() {\n    let Some(rs) = render_state() else { return };\n    let r = R::new(rs);\n    if !r.supports_bc() {\n        return;\n    }\n}").0, ["x.rs:6"]);
    // 7: a return in a nested fn is not the test's; 8: nor is one outside a test.
    assert!(hits(
        "#[test]\nfn t() {\n    fn walk(a: u8) {\n        return;\n    }\n    walk(1);\n}"
    )
    .0
    .is_empty());
    assert!(hits("fn t() {\n    return;\n}").0.is_empty());
    // 9: `return Ok(());` counts.
    assert_eq!(
        hits(
            "#[test]\nfn t() -> R {\n    if skip() {\n        return Ok(());\n    }\n    Ok(())\n}"
        )
        .0,
        ["x.rs:4"]
    );
    // 10: a print in a sibling branch does not explain the return.
    assert_eq!(hits("#[test]\nfn t() {\n    if a() {\n        eprintln!(\"a\");\n    } else if b() {\n        return;\n    }\n}").0, ["x.rs:6"]);
    // 11: a helper on an earlier statement excuses nothing; one on the line that opens the block does.
    assert_eq!(
        hits("#[test]\nfn t() {\n    let rs = render_state();\n    if other() { return; }\n}").0,
        ["x.rs:4"]
    );
    assert!(hits(
        "#[test]\nfn t() {\n    if encoders_sys::skip_byte_pins() {\n        return;\n    }\n}"
    )
    .0
    .is_empty());
    // 12: a wrapped attribute between #[test] and fn still marks a test.
    assert_eq!(hits("#[test]\n#[cfg_attr(\n    not(windows),\n    allow(unused)\n)]\nfn t() {\n    return;\n}").0, ["x.rs:7"]);
    // 13: a print three lines above explains it, four lines above does not.
    assert!(hits(
        "#[test]\nfn t() {\n    eprintln!(\"a\");\n    let a = 1;\n    let b = 2;\n    return;\n}"
    )
    .0
    .is_empty());
    assert_eq!(hits("#[test]\nfn t() {\n    eprintln!(\"a\");\n    let a = 1;\n    let b = 2;\n    let c = 3;\n    return;\n}").0, ["x.rs:7"]);
    // 14: `return }` counts.
    assert_eq!(
        hits("#[test]\nfn t() {\n    let Some(a) = f() else { return };\n}").0,
        ["x.rs:3"]
    );
    // 15: a helper is matched as itself, and not under a `!`.
    assert_eq!(
        hits("#[test]\nfn t() {\n    let Some(rs) = my_render_state() else { return };\n}").0,
        ["x.rs:3"]
    );
    assert_eq!(hits("#[test]\nfn t() {\n    if !encoders_sys::skip_byte_pins() {\n        return;\n    }\n}").0, ["x.rs:4"]);
    // 16: a nested fn in any form keeps its returns out of the test's.
    assert!(hits("#[test]\nfn t() {\n    pub(super) fn w() {\n        return;\n    }\n    const fn k() {\n        return;\n    }\n    unsafe fn u() {\n        return;\n    }\n}").0.is_empty());
}

/// The shared walker skips an absent root only when it is not required (AF-R2).
#[test]
fn the_walker_skips_only_an_absent_optional_root() {
    let absent = std::env::temp_dir().join("sdc-it-no-such-root");
    let mut out = Vec::new();
    assert!(rs_files(&absent, true, &mut out).is_err());
    assert!(rs_files(&absent, false, &mut out).is_ok() && out.is_empty());
}
