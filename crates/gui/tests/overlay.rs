//! The picture's placement in a preview pane (the picture's screen rect only, all nine copies with Tile).
use eframe::egui;

/// A1, K8: `uv_rect` is (offset, scale); the picture covers its screen rect only, all nine copies with Tile.
#[test]
fn the_picture_covers_its_rect_and_nothing_past_it() {
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
