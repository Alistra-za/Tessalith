#![windows_subsystem = "windows"]
//! `tessalith.exe`.
use engine::session::Session;
use sdc_gui::model::{AppModel, Dialog};
use sdc_gui::{app::SdcApp, fonts, layout, render, settings};
use std::sync::Arc;

fn build_model() -> AppModel {
    let path = settings::settings_path();
    let (s, err) = path
        .as_deref()
        .map(settings::GuiSettings::load)
        .unwrap_or_default();
    let mut model = match Session::new() {
        Ok(session) => {
            // Analyses are reused across runs (Note DZ); no `LOCALAPPDATA`, no store.
            let session = sdc_gui::with_analysis_cache(
                session,
                std::env::var_os("LOCALAPPDATA")
                    .as_deref()
                    .map(std::path::Path::new),
                &s,
            );
            let mut m = AppModel::new(s, path, Arc::new(session));
            m.user_config = engine::presets::user_config_path();
            m.user_presets = engine::presets::user_presets_dir();
            m.scan_db = sdc_gui::model::meshes::default_store_path();
            m.shader_cache = sdc_gui::model::viewport3d::default_cache_dir();
            m
        }
        Err(e) => {
            let mut m = AppModel::placeholder(s);
            m.dialog = Some(Dialog::Error(format!("cannot start the engine: {e}")));
            m
        }
    };
    if let Some(e) = err {
        model.keep_banner(format!("Settings were not read, defaults are used: {e}"));
    }
    model
}

fn open_startup_folder(
    model: &mut AppModel,
    args: impl IntoIterator<Item = std::ffi::OsString>,
    reopen: Option<Option<std::path::PathBuf>>,
) {
    if let Some(folder) = sdc_gui::crash_log::folder_arg(args).or_else(|| reopen.flatten()) {
        model.open_folder(folder);
    }
}

fn main() -> eframe::Result {
    sdc_gui::crash_log::install();
    // Note FC4: a restart after a lost graphics device reopens the folder that was open.
    let reopen = sdc_gui::crash_log::after_device_loss_folder(std::env::args_os().skip(1));
    sdc_gui::crash_log::mark_started(reopen.is_some());
    // The saved window (AG17, AG18), fitted to the monitors before the window exists.
    let saved = settings::settings_path()
        .map(|p| settings::GuiSettings::load(&p).0.layout)
        .unwrap_or_default();
    let win = layout::initial_window(&saved, &layout::monitors());
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title(sdc_gui::PRODUCT_NAME)
        .with_inner_size(win.size)
        .with_maximized(win.maximized);
    if let Some(p) = win.pos {
        viewport = viewport.with_position(p);
    }
    match eframe::icon_data::from_png_bytes(include_bytes!("../assets/tessalith-256.png")) {
        Ok(icon) => viewport = viewport.with_icon(icon),
        Err(e) => eprintln!("window icon skipped: {e}"),
    }
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        // BC texture compression and the adapter's own limits, for the preview textures.
        wgpu_options: render::wgpu_configuration(),
        viewport,
        // fonts::install_system_fonts installs the one system font provider (Note JM); eframe must not add its own.
        system_font_fallback: false,
        ..Default::default()
    };
    eframe::run_native(
        sdc_gui::PRODUCT_NAME,
        options,
        Box::new(|cc| {
            // Never fails: without the usual fonts the app starts on its built-in ones and says so (Note JM).
            let Ok(font_choice) = fonts::install_system_fonts(&cc.egui_ctx);
            if let Some(rs) = &cc.wgpu_render_state {
                sdc_gui::crash_log::watch_device(rs, sdc_gui::crash_log::run_log_path());
            }
            // No render state means no usable adapter: the app still starts, without a preview.
            let mut model = build_model();
            if let Some(text) = font_choice.message() {
                model
                    .messages
                    .log(sdc_gui::model::messages::Severity::Warning, text);
            }
            open_startup_folder(&mut model, std::env::args_os().skip(1), reopen);
            let app = SdcApp::new(model).with_render_state(cc.wgpu_render_state.as_ref());
            app.restore_ctx(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdc_gui::crash_log::after_device_loss_folder;

    #[test]
    fn folder_arg_wins_in_both_orders_and_a_bare_restart_stays_a_restart() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let explicit = a.path().as_os_str().to_owned();
        let previous = b.path().as_os_str().to_owned();
        let check = |args: &[std::ffi::OsString], restarted, want: Option<&std::path::Path>| {
            let reopen = after_device_loss_folder(args.iter().cloned());
            assert_eq!(reopen.is_some(), restarted);
            let mut model = AppModel::placeholder(settings::GuiSettings::default());
            open_startup_folder(&mut model, args.iter().cloned(), reopen);
            assert_eq!(model.root.as_deref(), want);
            assert_eq!(
                model.folder_epoch,
                u64::from(want.is_some()),
                "one open per folder"
            );
        };
        for args in [
            [
                explicit.clone(),
                "--after-device-loss".into(),
                previous.clone(),
            ],
            ["--after-device-loss".into(), previous, explicit.clone()],
        ] {
            check(&args, true, Some(a.path()));
        }
        check(&[explicit], false, Some(a.path()));
        check(&sdc_gui::crash_log::relaunch_args(None), true, None);
        check(
            &sdc_gui::crash_log::relaunch_args(Some(b.path())),
            true,
            Some(b.path()),
        );
    }
}
