//! Note JM5: a font panic caught on purpose is not a crash. The real hook (`crash_log::install_in`) writes no recovery
//! file for it and logs one line; an uncaught main-thread panic still writes them (Note AR). A `[[test]]` of its own:
//! it installs the process-wide panic hook. The hook acts only on a thread named "main", as in `crash_recovery.rs`.
use eframe::egui::{self, FontProvider as _};
use sdc_gui::{crash_log, fonts};

fn on_main(f: impl FnOnce() + Send + 'static) -> std::thread::Result<()> {
    std::thread::Builder::new()
        .name("main".into())
        .spawn(f)
        .unwrap()
        .join()
}

#[test]
fn a_caught_font_panic_writes_no_recovery_file() {
    let logs = tempfile::tempdir().unwrap();
    let docs = tempfile::tempdir().unwrap();
    let doc = docs.path().join("sdc.toml");
    crash_log::install_in(Some(logs.path().to_path_buf()));
    crash_log::set_recovery(vec![(doc.clone(), "edited".into())]);
    on_main(|| assert!(fonts::guarded(|| panic!("no DirectWrite")).1.lookup_failed)).unwrap();
    on_main(|| {
        let g = fonts::Guarded::new(
            |_: &egui::FallbackRequest<'_>| -> Option<egui::FontInsert> {
                panic!("enumeration thread gone")
            },
        );
        let req = egui::FallbackRequest {
            cluster: "日",
            family: &egui::FontFamily::Proportional,
        };
        assert!(g.font_for(&req).is_none() && g.font_for(&req).is_none());
    })
    .unwrap();
    assert!(
        !crash_log::recovery_path(&doc).exists(),
        "a caught font panic wrote a recovery file"
    );
    let log = std::fs::read_dir(logs.path())
        .unwrap()
        .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
        .collect::<String>();
    assert_eq!(
        log.lines()
            .filter(|l| l.contains("font lookup failed"))
            .count(),
        2,
        "one line per caught panic:\n{log}"
    );
    assert!(!log.contains("backtrace:"), "{log}");
    // Control: a real main-thread crash still keeps the unsaved edits.
    assert!(on_main(|| panic!("real crash")).is_err());
    assert!(crash_log::recovery_path(&doc).exists());
}
