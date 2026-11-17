//! The app's one reduced-motion switch (review E10, controller ruling). egui has no reduced-motion API, so the OS
//! setting is read once at start and kept in the context; any motion asks `reduced` before it fades or moves.
use eframe::egui;

/// Where `install` keeps the OS value (and tests their own, `force_reduced`): in the context, so two contexts never
/// share it.
fn key() -> egui::Id {
    egui::Id::unique("motion-os-reduced")
}

/// True when the OS asks for no animations, or the style has egui's own animations off (`animation_time` 0).
pub fn reduced(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(key())).unwrap_or(false)
        || ctx.global_style().animation_time == 0.0
}

/// Reads the OS value once and keeps it in `ctx`. When the OS asks for no animations, egui's own animations stop too
/// (`animation_time` 0 on both themes).
pub fn install(ctx: &egui::Context) {
    let os = os_reduced();
    ctx.data_mut(|d| d.insert_temp(key(), os));
    if os {
        ctx.all_styles_mut(|s| s.animation_time = 0.0);
    }
}

/// Windows' "Animation effects" setting (Settings > Accessibility > Visual effects): off means reduced. A failed call
/// reads as not reduced.
#[cfg(windows)]
fn os_reduced() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION,
    };
    let mut on: windows_sys::core::BOOL = 1;
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL through `pvparam`, which points at `on`.
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            (&mut on as *mut windows_sys::core::BOOL).cast(),
            0,
        )
    };
    ok != 0 && on == 0
}

#[cfg(not(windows))]
fn os_reduced() -> bool {
    false
}

/// Test seam: what the OS value reads as for this context.
#[cfg(feature = "test-hooks")]
pub fn force_reduced(ctx: &egui::Context, on: bool) {
    ctx.data_mut(|d| d.insert_temp(key(), on));
}
