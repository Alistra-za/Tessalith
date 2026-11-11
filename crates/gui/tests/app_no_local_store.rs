//! `SdcApp::new` must not read `%LOCALAPPDATA%` (hard rule: tests never touch it): the scan DB path comes from
//! `model.scan_db`, which only `main` sets.
mod support;
use sdc_gui::app::SdcApp;

#[test]
fn sdc_app_new_takes_the_scan_db_from_the_model_not_the_environment() {
    let d = tempfile::tempdir().unwrap();
    std::env::set_var("LOCALAPPDATA", d.path());
    let app = SdcApp::new(support::opened(d.path()));
    assert_eq!(app.meshes.store_path, None);
    assert_eq!(app.state.set3d.store_path, None);

    let mut m = support::opened(d.path());
    m.scan_db = Some(d.path().join("x.db"));
    let app = SdcApp::new(m);
    assert_eq!(app.meshes.store_path, Some(d.path().join("x.db")));
    assert_eq!(app.state.set3d.store_path, Some(d.path().join("x.db")));
}

#[test]
fn sdc_app_new_takes_the_shader_cache_from_the_model_not_the_environment() {
    let d = tempfile::tempdir().unwrap();
    std::env::set_var("LOCALAPPDATA", d.path());
    let app = SdcApp::new(support::opened(d.path()));
    assert_eq!(app.meshes.v3.cache_dir, None);
    assert_eq!(app.state.set3d.v3.cache_dir, None);

    let mut m = support::opened(d.path());
    m.shader_cache = Some(d.path().join("sc"));
    let app = SdcApp::new(m);
    assert_eq!(app.meshes.v3.cache_dir, Some(d.path().join("sc")));
    assert_eq!(app.state.set3d.v3.cache_dir, Some(d.path().join("sc")));
}

#[test]
fn sdc_app_new_creates_no_analysis_cache() {
    let d = tempfile::tempdir().unwrap();
    std::env::set_var("LOCALAPPDATA", d.path());
    let _app = SdcApp::new(support::opened(d.path()));
    assert!(!d.path().join("sdc").join("analysis-cache").exists());
}

/// Plans `textures` with `s`, so a store (if any) gets its first entries.
fn plan_once(s: &engine::session::Session, textures: &std::path::Path, out: &std::path::Path) {
    use engine::api::{BatchRequest, CancelToken};
    use engine::events::{Reporter, VecSink};
    let mut r = BatchRequest::new(
        vec![textures.to_path_buf()],
        out.to_path_buf(),
        engine::config::Target::Cs,
    );
    r.recursive = true;
    let rep = Reporter::new(std::sync::Arc::new(VecSink::default()), "t");
    s.plan(r, &rep, &CancelToken::default()).unwrap();
}

#[test]
fn with_analysis_cache_stores_under_the_given_folder_only() {
    let base = tempfile::tempdir().unwrap();
    let src = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    let textures = support::mod_folder(src.path());
    let new = || engine::session::Session::new().unwrap();
    let settings = sdc_gui::settings::GuiSettings::default();

    // No LOCALAPPDATA: no store, nothing written.
    plan_once(
        &sdc_gui::with_analysis_cache(new(), None, &settings),
        &textures,
        out.path(),
    );
    assert_eq!(std::fs::read_dir(base.path()).unwrap().count(), 0);

    // With a base folder the entries land under <base>\sdc\analysis-cache.
    let s = sdc_gui::with_analysis_cache(new(), Some(base.path()), &settings);
    plan_once(&s, &textures, out.path());
    let dir = base.path().join("sdc").join("analysis-cache");
    let count = || std::fs::read_dir(&dir).unwrap().count();
    let stored = count();
    assert!(stored > 0);

    // Note HV2: the folder is pruned at start to the limit in the settings; the default limit keeps these.
    let _s = sdc_gui::with_analysis_cache(new(), Some(base.path()), &settings);
    assert_eq!(count(), stored);
    let none = sdc_gui::settings::GuiSettings {
        analysis_cache_mb: Some(0),
        ..settings
    };
    let _s = sdc_gui::with_analysis_cache(new(), Some(base.path()), &none);
    assert_eq!(count(), 0);
}
