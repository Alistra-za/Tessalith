//! The preview overlay (DECISIONS Note CL44): UV coverage dims the texels no mesh uses. Built on a worker from the
//! cached mask, as a ready `ColorImage`, so the UI only uploads it. One job at a time, keyed by the item and the mask;
//! a result for an older key is dropped by its generation.
use super::uv_coverage::UvMask;
use super::AppModel;
use crate::worker::{or_send, spawn_guarded, WorkerMsg};
use eframe::egui::ColorImage;
use std::sync::{Arc, Weak};

/// What an overlay shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverlayKind {
    Uv,
}

/// UV coverage: a texel no mesh uses (dimmed) and one a mesh uses (a light veil).
pub const UV_UNUSED: [u8; 4] = [0, 0, 0, 180];
pub const UV_USED: [u8; 4] = [255, 255, 255, 40];

/// The UV coverage overlay of `mask` (row 0 at v = 0, the image's top), ready to upload. Built by an overlay job for
/// the expanded item (`AppModel::uv_overlay`, CL44): it depends on the mask only, never on the source mip.
pub fn uv_overlay_image(mask: &UvMask) -> Arc<ColorImage> {
    let n = mask.size;
    let mut rgba = Vec::with_capacity(n as usize * n as usize * 4);
    for y in 0..n {
        for x in 0..n {
            rgba.extend_from_slice(if mask.get(x, y) { &UV_USED } else { &UV_UNUSED });
        }
    }
    Arc::new(ColorImage::from_rgba_unmultiplied(
        [n as usize, n as usize],
        &rgba,
    ))
}

/// Builds the overlay image from a mask on the overlay worker.
pub type UvBuild = fn(&UvMask) -> Arc<ColorImage>;

/// The overlay job and the overlay on show.
#[derive(Default)]
pub struct OverlayState {
    /// The item the job in flight, or else `image`, was built for.
    pub key: Option<String>,
    /// The overlay on show, ready to upload. While a rebuild for the same item runs (another mask), the previous one
    /// stays (review M1); `None` when the job failed.
    pub image: Option<Arc<ColorImage>>,
    generation: u64,
    running: bool,
    /// The UV mask the key's image is built from: another answer (a new size, a re-read) builds again.
    mask: Weak<UvMask>,
}

impl OverlayState {
    /// Drop the overlay on show and the key (review M7). A job in flight still clears `running` when it lands.
    pub fn clear(&mut self) {
        self.key = None;
        self.image = None;
        self.mask = Weak::new();
    }

    /// A new job for `item`: its generation. Another item's overlay is never shown over this one.
    fn start(&mut self, item: &str) -> u64 {
        if self.key.as_deref() != Some(item) {
            self.image = None;
        }
        self.generation += 1;
        self.key = Some(item.to_string());
        self.running = true;
        self.generation
    }
}

impl AppModel {
    /// The UV overlay of the expanded `item` from `mask` (CL44): one overlay job per mask, so the UI thread does no
    /// per-texel work (CL29) and only the image on show is kept. The previous image stays while the same item rebuilds.
    pub(super) fn uv_overlay_job(
        &mut self,
        item: &str,
        mask: Arc<UvMask>,
    ) -> Option<Arc<ColorImage>> {
        self.start_uv_overlay(item, mask, uv_overlay_image)
    }

    /// `uv_overlay_job` with the image builder swapped (a test makes it slow or panicking).
    #[cfg(feature = "test-hooks")]
    pub fn uv_overlay_job_with(
        &mut self,
        item: &str,
        mask: Arc<UvMask>,
        build: UvBuild,
    ) -> Option<Arc<ColorImage>> {
        self.start_uv_overlay(item, mask, build)
    }

    fn start_uv_overlay(
        &mut self,
        item: &str,
        mask: Arc<UvMask>,
        build: UvBuild,
    ) -> Option<Arc<ColorImage>> {
        if self.preview.item.as_deref() != Some(item) {
            return None;
        }
        let o = &self.overlay;
        let same = o.key.as_deref() == Some(item);
        if same && o.mask.upgrade().is_some_and(|m| Arc::ptr_eq(&m, &mask)) {
            return o.image.clone();
        }
        let generation = self.overlay.start(item);
        self.overlay.mask = Arc::downgrade(&mask);
        spawn_guarded(self.tx.clone(), "overlay", move |tx| {
            // A panic lands as this job's "no overlay", so v2_jobs_pending clears (K9) and the key is not retried.
            let image = or_send(
                tx,
                || build(&mask),
                |_| WorkerMsg::Overlay {
                    generation,
                    image: None,
                },
            );
            let _ = tx.send(WorkerMsg::Overlay {
                generation,
                image: Some(image),
            });
        });
        self.overlay.image.clone()
    }

    pub(super) fn overlay_landed(&mut self, generation: u64, image: Option<Arc<ColorImage>>) {
        let o = &mut self.overlay;
        if generation == o.generation && o.running {
            o.running = false;
            // Cleared while it ran (toggle off, preview closed): drop the result.
            if o.key.is_some() {
                o.image = image;
            }
        }
    }

    /// An overlay job is in flight.
    pub fn overlay_pending(&self) -> bool {
        self.overlay.running
    }
}
