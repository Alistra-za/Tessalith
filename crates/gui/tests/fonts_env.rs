//! `SDC_FONT=embedded` (test-hooks only) installs the embedded floor (Note JM). A `[[test]]` of its own: it sets an
//! environment variable.
use eframe::egui;
use sdc_gui::fonts::{FontChoice, FontSource, FLOOR_NO_FONT};

#[test]
fn sdc_font_embedded_installs_the_floor() {
    std::env::set_var("SDC_FONT", "embedded"); // edition 2021 (root Cargo.toml:7), as app_no_local_store.rs:9

    // Never unset: this binary holds this one test.
    let ctx = egui::Context::default();
    let Ok(choice) = sdc_gui::fonts::install_system_fonts(&ctx);
    assert_eq!(
        choice,
        FontChoice {
            proportional: FontSource::Embedded,
            monospace: FontSource::Embedded,
            lookup_failed: false
        }
    );
    assert_eq!(choice.message(), Some(FLOOR_NO_FONT));
    // A dropped `FullOutput` with unapplied texture deltas panics in a debug build (mistakes/gui.md).
    ctx.run_ui(Default::default(), |_| {})
        .textures_delta
        .clear();
    let first = ctx.fonts(|f| f.definitions().families[&egui::FontFamily::Proportional][0].clone());
    assert_eq!(first, "Ubuntu-Light");
}
