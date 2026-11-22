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
        Ok(mut session) => {
            session.set_profile_out(
                std::env::var_os("SDC_PROFILE")
                    .filter(|v| !v.is_empty())
                    .map(std::path::PathBuf::from),
            );
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

/// An invalid explicit folder reports its error even on a device-loss restart.
fn open_startup_folder(
    model: &mut AppModel,
    args: impl IntoIterator<Item = std::ffi::OsString>,
    reopen: Option<Option<std::path::PathBuf>>,
) {
    use sdc_gui::crash_log::{checked_folder, folder_arg};
    let folder = match folder_arg(args) {
        Ok(None) => reopen.flatten().map(checked_folder).transpose(),
        explicit => explicit,
    };
    match folder {
        Ok(Some(path)) => model.open_folder(path),
        Ok(None) => {}
        Err(message) => {
            model
                .messages
                .log(sdc_gui::model::messages::Severity::Error, &message);
            model.status_line = message;
        }
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

    fn bad_folder(path: &std::path::Path) {
        let reason = std::fs::read_dir(path).unwrap_err().to_string();
        let mut model = AppModel::placeholder(settings::GuiSettings::default());
        open_startup_folder(&mut model, [path.as_os_str().to_owned()], None);
        assert!(model.root.is_none(), "the start card needs an empty root");
        assert!(model.settings.recent.is_empty());
        assert!(
            model.status_line.contains(&path.display().to_string())
                && model.status_line.contains(&reason),
            "missing path or OS reason in status: {:?}",
            model.status_line
        );
        assert!(model
            .messages
            .entries()
            .any(|e| e.text == model.status_line));
    }

    #[test]
    fn folder_arg_reports_a_missing_folder() {
        let dir = tempfile::tempdir().unwrap();
        bad_folder(&dir.path().join("missing"));
    }

    #[test]
    fn folder_arg_reports_a_regular_file() {
        let file = tempfile::NamedTempFile::new().unwrap();
        bad_folder(file.path());
    }

    #[cfg(windows)]
    #[test]
    fn folder_arg_reports_an_unlistable_folder() {
        let dir = tempfile::tempdir().unwrap();
        let acl = |args: &[&str]| {
            assert!(std::process::Command::new("icacls")
                .arg(dir.path())
                .args(args)
                .output()
                .unwrap()
                .status
                .success());
        };
        acl(&["/deny", "*S-1-1-0:(RD)"]);
        let check = std::panic::catch_unwind(|| {
            assert!(dir.path().is_dir());
            bad_folder(dir.path());
        });
        acl(&["/remove:d", "*S-1-1-0"]);
        if let Err(panic) = check {
            std::panic::resume_unwind(panic);
        }
    }

    #[test]
    fn folder_arg_stores_dot_as_an_absolute_recent_folder() {
        const CHILD: &str = "SDC_TEST_STARTUP_FOLDER_CWD";
        let Some(folder) = std::env::var_os(CHILD) else {
            let dir = tempfile::tempdir().unwrap();
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "tests::folder_arg_stores_dot_as_an_absolute_recent_folder",
                ])
                .env(CHILD, dir.path())
                .current_dir(dir.path())
                .status()
                .unwrap();
            assert!(status.success());
            assert_eq!(
                settings::GuiSettings::load(&dir.path().join("gui.toml"))
                    .0
                    .recent,
                [dir.path()]
            );
            return;
        };
        let folder = std::path::PathBuf::from(folder);
        assert_eq!(std::env::current_dir().unwrap(), folder);
        let saved = folder.join("gui.toml");
        let mut model = AppModel::placeholder(settings::GuiSettings::default());
        model.settings_path = Some(saved.clone());
        open_startup_folder(&mut model, [".".into()], None);
        assert_eq!(
            settings::GuiSettings::load(&saved).0.recent,
            [folder.as_path()]
        );
        assert_eq!(model.settings.recent, [folder.as_path()]);
    }

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
        check(&[], false, None);
        check(&sdc_gui::crash_log::relaunch_args(None), true, None);
        check(
            &sdc_gui::crash_log::relaunch_args(Some(b.path())),
            true,
            Some(b.path()),
        );
    }
}
