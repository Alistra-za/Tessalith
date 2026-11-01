//! Note GI (XPMF Task 8, design X10): the XPMF profile in the MATO preview layer. Model level: the layer and the
//! scene's texture map are read back from the viewport, as in `set_preview_mato.rs`.
mod support;
use render3d::ProjectedLayer;
use sdc_gui::model::cs_pbr::Kind;
use sdc_gui::model::set_preview::*;
use sdc_gui::model::viewport3d::{Content, FileSource};
use sdc_gui::model::AppModel;
use serde_json::{json, Map};
use std::path::Path;
use std::time::{Duration, Instant};

const ID: &str = "landscape/rock01";
const NOISE: &str = "textures/effects/projectednoise.dds";

fn layer() -> ProjectedLayer {
    layer_from_block(&Map::new(), None)
}

#[test]
fn apply_profile_overrides_coverage() {
    // No DATA: 0.35, 0.4 and 48.
    let mut l = layer();
    assert_eq!(
        (l.falloff_scale, l.falloff_bias, l.noise_uv_scale),
        (0.35, 0.4, 48.0)
    );
    let doc = json!({"falloffScale": 0.5, "noiseUVScale": 24})
        .as_object()
        .unwrap()
        .clone();
    apply_profile(&mut l, &doc);
    assert_eq!(
        (l.falloff_scale, l.falloff_bias, l.noise_uv_scale),
        (0.5, 0.4, 24.0)
    );
    // A hand-edited value the renderer cannot use keeps the record's.
    for bad in [
        json!({"noiseUVScale": 0}),
        json!({"noiseUVScale": -3}),
        json!({"falloffBias": "x"}),
    ] {
        let mut l = layer();
        apply_profile(&mut l, bad.as_object().unwrap());
        assert_eq!(l, layer(), "{bad}");
    }
}

/// `support::pbr_opened` (rock01 is a PBR Landscape set) with a noise map, a Mato block and an XPMF block.
fn opened(dir: &Path) -> (AppModel, SetPreview) {
    let r = dir.join("textures/landscape");
    std::fs::create_dir_all(&r).unwrap();
    image::ImageBuffer::from_fn(64, 64, |x, y| image::Luma([((x + y) * 2) as u8]))
        .save(r.join("rock01_noise.png"))
        .unwrap();
    let mut m = support::pbr_opened(dir);
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
    m.add_cs_block(ID, Kind::Mato);
    m.set_cs_edids(ID, Kind::Mato, &["SnowMat1P".to_string()]);
    let key = sdc_gui::model::sets::set_views(&m)
        .into_iter()
        .find(|v| v.id == ID)
        .unwrap()
        .key;
    m.select_set(&key);
    (m, SetPreview::new(None, None, None, DEFAULT_CAP))
}

fn frame(s: &mut SetPreview, m: &AppModel) {
    s.shown = true;
    s.input = true;
    s.poll(m);
}

fn run(s: &mut SetPreview, m: &AppModel) {
    let t = Instant::now();
    loop {
        frame(s, m);
        if !s.busy() {
            return;
        }
        assert!(
            t.elapsed() < Duration::from_secs(90),
            "the preview did not settle"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn noise_source(s: &SetPreview) -> Option<FileSource> {
    let Some(Content::Builtin { textures, .. }) = s.v3.content() else {
        panic!("{:?}", s.v3.content())
    };
    textures.get(NOISE).cloned()
}

#[test]
fn profile_textures_feed_the_layer() {
    let dir = tempfile::tempdir().unwrap();
    let (mut m, mut s) = opened(dir.path());
    s.set_mato_layer(true);
    run(&mut s, &m);
    assert_eq!(noise_source(&s), None, "no block: the load order has none");
    m.set_xpmf_on(ID, true);
    run(&mut s, &m);
    // The profile's default noiseTexture is the set's noise output; the layer gets this session's bytes for it.
    let own = s.enc.memory_sources();
    let (_, bytes) = own
        .iter()
        .find(|(p, _)| p.ends_with("noise.dds"))
        .expect("the noise member is encoded");
    assert_eq!(noise_source(&s).as_ref(), Some(bytes));
    assert!(matches!(noise_source(&s), Some(FileSource::Memory { .. })));
    // Coverage comes from the block too.
    m.set_xpmf_value(ID, "falloffScale", Some(json!(0.9)));
    run(&mut s, &m);
    assert_eq!(s.v3.projected().map(|l| l.falloff_scale), Some(0.9));
}

#[test]
fn texture_change_cycles_layer() {
    let dir = tempfile::tempdir().unwrap();
    let (mut m, mut s) = opened(dir.path());
    m.set_xpmf_on(ID, true);
    s.set_mato_layer(true);
    run(&mut s, &m);
    assert!(s.v3.projected().is_some());
    assert!(noise_source(&s).is_some());
    // A value edit leaves the layer on.
    m.set_xpmf_value(ID, "falloffBias", Some(json!(0.7)));
    frame(&mut s, &m);
    assert_eq!(s.v3.projected().map(|l| l.falloff_bias), Some(0.7));
    // A new noise path: None first, then Some with the new texture map.
    m.set_xpmf_value(
        ID,
        "noiseTexture",
        Some(json!("textures/landscape/elsewhere.dds")),
    );
    frame(&mut s, &m);
    assert_eq!(s.v3.projected(), None, "off for one frame");
    run(&mut s, &m);
    assert!(s.v3.projected().is_some(), "on again");
    assert_eq!(
        noise_source(&s),
        None,
        "a path nothing serves leaves the plain fallback"
    );
    // Turned off: no layer, and nothing waits for a scene push.
    s.set_mato_layer(false);
    m.set_xpmf_value(
        ID,
        "noiseTexture",
        Some(json!("textures/landscape/other.dds")),
    );
    frame(&mut s, &m);
    s.set_mato_layer(true);
    run(&mut s, &m);
    assert!(s.v3.projected().is_some());
}
