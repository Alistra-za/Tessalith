//! Note GI (XPMF Task 8, design X10): the XPMF profile in the MATO preview layer. Model level: the layer and the
//! scene's texture map are read back from the viewport, as in `set_preview_mato.rs`; one test runs the real render
//! thread on WARP with the repo's stand-in shader.
#[path = "../../esp/tests/common/mod.rs"]
mod b;
#[path = "../../modscan/tests/common/mod.rs"]
mod scan_fixture;
mod support;
use render3d::ProjectedLayer;
use sdc_gui::model::cs_pbr::Kind;
use sdc_gui::model::set_preview::*;
use sdc_gui::model::sets::{set_views, SlotFill, SlotKind};
use sdc_gui::model::viewport3d::{Content, FileSource, V3Reply};
use sdc_gui::model::AppModel;
use serde_json::{json, Map};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const ID: &str = "landscape/rock01";
const DIFFUSE: &str = "textures/effects/projecteddiffuse.dds";
const NORMAL: &str = "textures/effects/projectednormal.dds";
const DETAIL: &str = "textures/effects/projectednormaldetail.dds";
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

/// `support::pbr_opened` (rock01 is a PBR Landscape set) with a noise map and a detail normal, a Mato block and
/// the preview following the set.
fn opened(dir: &Path, db: Option<PathBuf>) -> (AppModel, SetPreview) {
    let r = dir.join("textures/landscape");
    std::fs::create_dir_all(&r).unwrap();
    image::ImageBuffer::from_fn(64, 64, |x, y| image::Luma([((x + y) * 2) as u8]))
        .save(r.join("rock01_noise.png"))
        .unwrap();
    image::ImageBuffer::from_pixel(64, 64, image::Rgb([128u8, 128, 255]))
        .save(r.join("rock01_detail.png"))
        .unwrap();
    let mut m = support::pbr_opened(dir);
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
    m.add_cs_block(ID, Kind::Mato);
    m.set_cs_edids(ID, Kind::Mato, &["SnowMat1P".to_string()]);
    let key = set_views(&m).into_iter().find(|v| v.id == ID).unwrap().key;
    m.select_set(&key);
    (m, SetPreview::new(None, None, db, DEFAULT_CAP))
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

fn source(s: &SetPreview, canonical: &str) -> Option<FileSource> {
    let Some(Content::Builtin { textures, .. }) = s.v3.content() else {
        panic!("{:?}", s.v3.content())
    };
    textures.get(canonical).cloned()
}

fn noise_item(m: &AppModel) -> String {
    let v = set_views(m).into_iter().find(|v| v.id == ID).unwrap();
    match v.slots.get(&SlotKind::ProjectedNoise) {
        Some(SlotFill::Own { item }) => item.clone(),
        other => panic!("{other:?}"),
    }
}

/// What the preview serves for a slot kind's own member.
fn member_bytes(s: &SetPreview, kind: SlotKind) -> FileSource {
    let path = s.enc.members[&kind].slot.paths[0].clone();
    s.enc.memory_sources()[&path].clone()
}

#[test]
fn profile_textures_feed_the_layer() {
    let dir = tempfile::tempdir().unwrap();
    let (mut m, mut s) = opened(dir.path(), None);
    s.set_mato_layer(true);
    run(&mut s, &m);
    for c in [DIFFUSE, NORMAL, DETAIL, NOISE] {
        assert_eq!(source(&s, c), None, "no block: the load order has none");
    }
    m.set_xpmf_on(ID, true);
    run(&mut s, &m);
    // The profile's defaults are the set's own outputs; each key feeds its register with this session's bytes.
    for (canonical, kind) in [
        (DIFFUSE, SlotKind::Diffuse),
        (NORMAL, SlotKind::Normal),
        (DETAIL, SlotKind::DetailNormal),
        (NOISE, SlotKind::ProjectedNoise),
    ] {
        let got = source(&s, canonical);
        assert!(
            matches!(got, Some(FileSource::Memory { .. })),
            "{canonical}"
        );
        assert_eq!(got, Some(member_bytes(&s, kind)), "{canonical}");
    }
    // Coverage comes from the block too.
    m.set_xpmf_value(ID, "falloffScale", Some(json!(0.9)));
    run(&mut s, &m);
    assert_eq!(s.v3.projected().map(|l| l.falloff_scale), Some(0.9));
}

#[test]
fn texture_change_cycles_layer() {
    let dir = tempfile::tempdir().unwrap();
    let (mut m, mut s) = opened(dir.path(), None);
    m.set_xpmf_on(ID, true);
    s.set_mato_layer(true);
    run(&mut s, &m);
    assert!(s.v3.projected().is_some());
    assert!(source(&s, NOISE).is_some());
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
        source(&s, NOISE),
        None,
        "a path nothing serves leaves the plain fallback"
    );
    // The grey line names the path the profile wrote, not the game's projected texture.
    s.v3.handle(V3Reply::ProjectedMissing(vec![NOISE.into()]));
    let lines = s.mato_lines();
    assert!(
        lines.contains(&mato_missing_text("textures/landscape/elsewhere.dds")),
        "{lines:?}"
    );
    assert!(
        lines.iter().all(|l| !l.contains("projectednoise")),
        "{lines:?}"
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

/// Final review M4: a texture change on the frame after a cycle's scene push must not turn the layer on. The scene
/// push of the frame before has set the resume flag, but the new map is only pushed later in the frame, so a layer
/// sent on now would be resolved against the old one (the model's readback after the frame cannot tell, hence the
/// layer itself is checked): each frame that changes the noise path ends with the layer off.
#[test]
fn a_texture_change_right_after_the_scene_push_keeps_the_layer_off() {
    let dir = tempfile::tempdir().unwrap();
    let (mut m, mut s) = opened(dir.path(), None);
    m.set_xpmf_on(ID, true);
    s.set_mato_layer(true);
    run(&mut s, &m);
    assert!(s.v3.projected().is_some());
    assert!(source(&s, NOISE).is_some(), "the default is the own member");
    let mut default_path = true;
    for i in 0..8 {
        // Alternate between a path nothing serves and the default (the own member), one change per frame.
        let v = (i % 2 == 0).then(|| json!("textures/landscape/elsewhere.dds"));
        default_path = v.is_none();
        m.set_xpmf_value(ID, "noiseTexture", v);
        frame(&mut s, &m);
        assert_eq!(s.v3.projected(), None, "frame {i}: the layer is on");
        assert_eq!(source(&s, NOISE).is_some(), default_path, "frame {i}");
    }
    run(&mut s, &m);
    assert!(s.v3.projected().is_some(), "on again at the end");
    assert_eq!(source(&s, NOISE).is_some(), default_path);
}
/// A new encode of the member the noise key names is a new texture: the layer takes it without being toggled.
#[test]
fn re_encoding_the_own_member_refreshes_the_texture() {
    let dir = tempfile::tempdir().unwrap();
    let (mut m, mut s) = opened(dir.path(), None);
    m.set_xpmf_on(ID, true);
    s.set_mato_layer(true);
    run(&mut s, &m);
    let before = source(&s, NOISE);
    assert!(matches!(before, Some(FileSource::Memory { .. })));
    // Two mip levels change the noise map's output, so the member encodes again.
    let item = noise_item(&m);
    m.set_field(&item, "mips.levels", json!(2));
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
    // Final review M3: the layer is on until the new encode is shown, goes off on some frame after it, then comes
    // on again. Only that None to Some step makes the renderer read the new texture; the new bytes alone in the map
    // would not.
    let shown = |s: &SetPreview| {
        s.enc.members[&SlotKind::ProjectedNoise]
            .state
            .shown()
            .map(|e| e.id)
    };
    let old_id = shown(&s);
    let t = Instant::now();
    let (mut landed, mut on_before, mut off_after) = (false, false, false);
    loop {
        frame(&mut s, &m);
        landed |= shown(&s) != old_id;
        on_before |= !landed && s.v3.projected().is_some();
        off_after |= landed && s.v3.projected().is_none();
        if !s.busy() {
            break;
        }
        assert!(
            t.elapsed() < Duration::from_secs(90),
            "the preview did not settle"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(landed, "the new encode was never shown");
    assert!(on_before, "the layer was not on before the encode");
    assert!(off_after, "the layer was not pushed off after the encode");
    assert!(s.v3.projected().is_some());
    let after = source(&s, NOISE);
    assert_ne!(after, before, "the new bytes reached the layer");
    assert_eq!(after, Some(member_bytes(&s, SlotKind::ProjectedNoise)));
}

/// A path the set does not write is looked up in the load order (the scan's winning copy).
#[test]
fn load_order_path_feeds_the_layer() {
    let inst = scan_fixture::instance_fixture("+A\n");
    let root = inst.path();
    scan_fixture::put(
        root,
        "Game/Data/Skyrim.esm",
        &b::plugin(esp::FLAG_ESM, &[], &[]),
    );
    scan_fixture::put(
        root,
        "mods/A/textures/landscape/loadorder_noise.dds",
        b"DDS stand-in",
    );
    scan_fixture::put(root, "profiles/Default/plugins.txt", b"");
    let db = root.join("scan.db");
    let mut st = modscan::store::Store::open(&db).unwrap();
    let src = modscan::scan::Source::Mo2(modscan::mo2::Instance::load(root, None).unwrap());
    modscan::scan::run(
        &src,
        &mut st,
        &std::sync::atomic::AtomicBool::new(false),
        &|_| {},
        &|| vec![],
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (mut m, mut s) = opened(dir.path(), Some(db));
    m.scan_db = Some(root.join("scan.db"));
    m.poll();
    m.set_xpmf_on(ID, true);
    m.set_xpmf_value(
        ID,
        "noiseTexture",
        Some(json!("Textures\\Landscape\\LoadOrder_Noise.dds")),
    );
    s.set_mato_layer(true);
    run(&mut s, &m);
    let got = source(&s, NOISE);
    assert!(
        got.is_some() && !matches!(got, Some(FileSource::Memory { .. })),
        "{got:?}"
    );
}

/// The render thread: an off and an on that arrive in one batch still read the textures again.
#[test]
fn off_and_on_in_one_batch_resolve_the_textures_again() {
    use sdc_gui::model::viewport3d::Viewport3d;
    let standin = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../render3d/tests/fixtures/standin"
    ));
    let f = render3d::locate::fingerprint(&standin, None).unwrap();
    let cache = tempfile::tempdir().unwrap();
    let mut v = Viewport3d::new(Some(f.clone()));
    v.on_located(Ok(Some(f)));
    v.cache_dir = Some(cache.path().to_path_buf());
    let mut mat = nif::material::MaterialState::builtin_default();
    // DO_ALPHA_TEST: the stand-in's pixel permutation compiles.
    mat.alpha = Some(nif::props::Alpha {
        flags: 0x200,
        threshold: 0,
    });
    let white = engine::image::Rgba32F {
        width: 4,
        height: 4,
        px: vec![[1.0; 4]; 16],
    };
    let dds = std::sync::Arc::new(uncompressed_dds(&[white], false, false).unwrap());
    let memory = |id| FileSource::Memory {
        id,
        bytes: dds.clone(),
    };
    let content = |key: u64, textures: Vec<(&str, FileSource)>| {
        Some(Content::Builtin {
            key,
            shape: render3d::builtin::Builtin::Sphere,
            material: mat.clone(),
            textures: std::sync::Arc::new(
                textures
                    .into_iter()
                    .map(|(p, s)| (p.to_string(), s))
                    .collect(),
            ),
        })
    };
    let wait = |v: &mut Viewport3d, want: &[&str]| {
        let t = Instant::now();
        while v.projected_missing != want {
            v.poll();
            v.request_frame(64, 64);
            assert!(
                t.elapsed().as_secs() < 60,
                "stuck at {:?}, wanted {want:?}",
                v.projected_missing
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    v.set_content(content(1, vec![(DIFFUSE, memory(1))]), false);
    v.set_projected_layer(Some(layer()));
    wait(&mut v, &[NORMAL, DETAIL, NOISE]);
    // New content with a noise texture, then an off and an on while it loads: one batch.
    v.set_content(
        content(2, vec![(DIFFUSE, memory(1)), (NOISE, memory(2))]),
        false,
    );
    v.request_frame(64, 64);
    v.set_projected_layer(None);
    v.set_projected_layer(Some(layer()));
    wait(&mut v, &[NORMAL, DETAIL]);
}
