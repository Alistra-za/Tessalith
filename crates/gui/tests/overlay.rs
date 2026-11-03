//! The preview overlay (Note CL44): its job (built off the UI thread, a stale result dropped, a panic cleared, freed on
//! collapse) and its placement (the picture's screen rect only, all nine copies with Tile). The image itself and its
//! upload are pinned in `uv_coverage.rs`; the app's poll in `v2_jobs_poll.rs`. The job is driven with a mask and a
//! swapped builder (`uv_overlay_job_with`), so no scan database is needed.
mod support;
use eframe::egui::{self, Color32, ColorImage};
use sdc_gui::model::uv_coverage::UvMask;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

const A: &str = "test/wall.png";
const B: &str = "test/wall_rmaos.png";

fn filled(c: Color32) -> Arc<ColorImage> {
    Arc::new(ColorImage::filled([4, 4], c))
}

fn slow(_: &UvMask) -> Arc<ColorImage> {
    std::thread::sleep(Duration::from_millis(1000));
    filled(Color32::RED)
}

static BUILT_OFF_UI: AtomicBool = AtomicBool::new(false);

fn noting_the_thread(_: &UvMask) -> Arc<ColorImage> {
    if std::thread::current().name() == Some("tessalith-overlay") {
        BUILT_OFF_UI.store(true, Ordering::SeqCst);
    }
    filled(Color32::GREEN)
}

fn panics(_: &UvMask) -> Arc<ColorImage> {
    panic!("overlay test panic")
}

fn mask() -> Arc<UvMask> {
    Arc::new(UvMask::new(8))
}

fn model(d: &std::path::Path) -> sdc_gui::model::AppModel {
    support::pbr_wall(d, 128, [128, 10, 200])
}

#[test]
fn the_overlay_is_built_off_the_ui_thread_and_a_stale_result_is_dropped() {
    let d = tempfile::tempdir().unwrap();
    let mut m = model(d.path());
    // Not the expanded item: nothing starts.
    assert!(m.uv_overlay_job_with(A, mask(), slow).is_none());
    assert!(!m.overlay_pending());
    m.expand(A);
    assert!(m.wait_idle(Duration::from_secs(60)));
    // A's job is slow; B's starts before it lands, so A's result arrives last and must be dropped.
    assert!(m.uv_overlay_job_with(A, mask(), slow).is_none());
    assert!(m.overlay_pending());
    m.expand(B);
    assert!(m.overlay_pending(), "A is still in flight");
    let mb = mask();
    assert!(m
        .uv_overlay_job_with(B, mb.clone(), noting_the_thread)
        .is_none());
    assert!(m.wait_idle(Duration::from_secs(10)));
    std::thread::sleep(Duration::from_millis(1200));
    assert!(m.wait_idle(Duration::from_secs(10)));
    let img = m.overlay.image.clone().expect("B's overlay");
    assert!(
        img.pixels.iter().all(|p| *p == Color32::GREEN),
        "A's stale image is on show"
    );
    assert!(
        BUILT_OFF_UI.load(Ordering::SeqCst),
        "built on the overlay worker"
    );
    assert_eq!(m.overlay.key.as_deref(), Some(B));
    // The same mask starts nothing new and hands back the image on show.
    let again = m.uv_overlay_job_with(B, mb, panics).expect("kept");
    assert!(Arc::ptr_eq(&again, &img) && !m.overlay_pending());
}

/// K9: a panicking overlay job never leaves `v2_jobs_pending()` set (the app would poll at 50 ms forever).
#[test]
fn a_panicking_overlay_job_clears_v2_jobs_pending() {
    let d = tempfile::tempdir().unwrap();
    let mut m = model(d.path());
    m.expand(A);
    assert!(m.wait_idle(Duration::from_secs(60)));
    let ma = mask();
    assert!(m.uv_overlay_job_with(A, ma.clone(), panics).is_none());
    assert!(
        m.wait_idle(Duration::from_secs(10)),
        "wait_idle spun to its timeout"
    );
    assert!(!m.v2_jobs_pending());
    // No image, and the same mask is not retried every frame.
    assert!(m.overlay.image.is_none());
    assert!(m.uv_overlay_job_with(A, ma, slow).is_none());
    assert!(!m.overlay_pending());
}

/// Review M7: collapsing frees the image and the key, and a job still running lands as a no-op.
#[test]
fn collapse_frees_the_overlay_and_drops_a_job_in_flight() {
    let d = tempfile::tempdir().unwrap();
    let mut m = model(d.path());
    m.expand(A);
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(m
        .uv_overlay_job_with(A, mask(), noting_the_thread)
        .is_none());
    assert!(m.wait_idle(Duration::from_secs(10)));
    assert!(m.overlay.image.is_some() && m.overlay.key.is_some());
    m.collapse();
    assert!(m.overlay.image.is_none() && m.overlay.key.is_none());
    m.expand(A);
    assert!(m.uv_overlay_job_with(A, mask(), slow).is_none());
    m.collapse();
    assert!(m.wait_idle(Duration::from_secs(10)));
    assert!(
        m.overlay.image.is_none(),
        "a result landed after the collapse"
    );
    assert!(!m.overlay_pending());
}

/// A1, K8: `uv_rect` is (offset, scale); the overlay covers the picture's screen rect only, all nine copies with Tile.
#[test]
fn the_overlay_covers_the_picture_and_nothing_past_it() {
    use sdc_gui::model::view::ViewState;
    use sdc_gui::ui::preview::picture_rects;
    let pane = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(200.0, 100.0));
    let mut v = ViewState::default(); // zoom 1, centred
    let (pic, uv) = picture_rects(&v, pane, [200.0, 100.0], (100, 50));
    assert_eq!(
        pic,
        egui::Rect::from_min_max(egui::pos2(60.0, 45.0), egui::pos2(160.0, 95.0))
    );
    assert_eq!(
        uv,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0))
    );
    v.tile = true;
    let (pic, uv) = picture_rects(&v, pane, [200.0, 100.0], (100, 50));
    assert_eq!(
        pic,
        egui::Rect::from_min_max(egui::pos2(-40.0, -5.0), egui::pos2(260.0, 145.0))
    );
    assert_eq!(
        uv,
        egui::Rect::from_min_max(egui::pos2(-1.0, -1.0), egui::pos2(2.0, 2.0))
    );
    // Panned and zoomed: the texel at the pane's centre is the view's centre uv.
    v.tile = false;
    v.zoom = 2.0;
    v.center = [0.25, 0.5];
    let (pic, _) = picture_rects(&v, pane, [200.0, 100.0], (100, 50));
    let u = (pane.center().x - pic.min.x) / pic.width();
    assert!((u - 0.25).abs() < 1e-6, "{u}");
    assert!((pic.width() - 200.0).abs() < 1e-4);
}
