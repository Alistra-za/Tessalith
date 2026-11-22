//! Note FI1: a skipped DDS input's tile shows its stored image, decoded by the GUI off the UI thread.
mod support;
use engine::config::Format;
use engine::dds::{dds_bytes, DdsDesc, ResolvedHeader};
use sdc_gui::model::preview::dds_thumbnail;
use std::path::Path;
use std::time::{Duration, Instant};
use support::*;

/// A pure-red `size` square DDS of `format` with `levels` mips (written by the engine's writer, never copied).
fn red_dds(path: &Path, format: Format, size: u32, levels: usize) {
    let desc = DdsDesc {
        width: size,
        height: size,
        format,
        srgb: false,
        header: ResolvedHeader::Legacy,
    };
    let data: Vec<Vec<u8>> = (0..levels)
        .map(|i| {
            let s = (size >> i).max(1) as usize;
            match format {
                // One BC1 block: both endpoints 0xF800 (565 red), every index 0.
                Format::Bc1 => [0x00, 0xF8, 0x00, 0xF8, 0, 0, 0, 0].repeat(s.div_ceil(4).pow(2)),
                // `Format::Rgba8` is written as B8G8R8A8 (DECISIONS S4).
                _ => [0, 0, 255, 255].repeat(s * s),
            }
        })
        .collect();
    std::fs::write(path, dds_bytes(&desc, &data).unwrap()).unwrap();
}

fn assert_red_thumb(path: &Path) {
    let t = dds_thumbnail(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(t.width.max(t.height) <= 128, "{}x{}", t.width, t.height);
    assert_eq!(t.rgba.len(), (t.width * t.height * 4) as usize);
    let c = (((t.height / 2) * t.width + t.width / 2) * 4) as usize;
    let px = &t.rgba[c..c + 3];
    for (got, want) in px.iter().zip([255u8, 0, 0]) {
        assert!(got.abs_diff(want) <= 2, "{}: {px:?}", path.display());
    }
}

#[test]
fn dds_thumbnail_is_at_most_128_and_rgba8() {
    let d = tempfile::tempdir().unwrap();
    let bc1 = d.path().join("bc1.dds");
    red_dds(&bc1, Format::Bc1, 256, 9);
    assert_red_thumb(&bc1);
    let rgba = d.path().join("rgba.dds");
    red_dds(&rgba, Format::Rgba8, 256, 9);
    assert_red_thumb(&rgba);
}

#[test]
fn a_mipless_512_dds_gets_a_thumbnail() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("one.dds");
    red_dds(&p, Format::Rgba8, 512, 1);
    assert_red_thumb(&p);
}

/// The mod folder plus a skipped `landscape/stored.dds`, opened, with its thumbnail landed.
fn opened_with_dds(dir: &Path) -> sdc_gui::model::AppModel {
    let root = mod_folder(dir);
    red_dds(&root.join("landscape/stored.dds"), Format::Bc1, 256, 9);
    let mut m = opened(dir);
    let end = Instant::now() + Duration::from_secs(30);
    while m.item("landscape/stored.dds").unwrap().thumb.is_none() && Instant::now() < end {
        m.poll();
        std::thread::sleep(Duration::from_millis(10));
    }
    m
}

#[test]
fn a_skipped_dds_tile_gets_its_thumbnail_after_the_plan() {
    let d = tempfile::tempdir().unwrap();
    let m = opened_with_dds(d.path());
    let item = m.item("landscape/stored.dds").unwrap();
    assert!(matches!(
        item.status,
        sdc_gui::model::items::ItemStatus::Skipped(_)
    ));
    assert!(item.thumb.is_some());
}

#[test]
fn a_replan_reuses_the_cached_thumbnail() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened_with_dds(d.path());
    let asked = m.dds_thumb_requests;
    assert!(asked >= 1);
    m.replan();
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(m.dds_thumb_requests, asked);
    assert!(m.item("landscape/stored.dds").unwrap().thumb.is_some());
}

/// Polls until no tile image is being decoded.
fn wait_thumbs(m: &mut sdc_gui::model::AppModel) {
    let end = Instant::now() + Duration::from_secs(30);
    while m.dds_thumbs_pending() && Instant::now() < end {
        m.poll();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!m.dds_thumbs_pending(), "tile images still decoding");
}

/// A BC1 header relabelled DXT3 (BC2, which the decoder refuses) with zeroed DXT3-sized data.
fn bc2_dds(path: &Path, size: u32, levels: usize) {
    red_dds(path, Format::Bc1, size, levels);
    let mut bytes = std::fs::read(path).unwrap();
    assert_eq!(&bytes[84..88], b"DXT1");
    bytes[84..88].copy_from_slice(b"DXT3");
    bytes.truncate(128);
    let data: usize = (0..levels)
        .map(|i| ((size >> i).max(1) as usize).div_ceil(4).pow(2) * 16)
        .sum();
    bytes.resize(128 + data, 0);
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn a_refused_dds_is_read_once_until_it_changes() {
    let d = tempfile::tempdir().unwrap();
    let root = mod_folder(d.path());
    let p = root.join("landscape/stored.dds");
    bc2_dds(&p, 256, 9);
    let mut m = opened(d.path());
    wait_thumbs(&mut m);
    let asked = m.dds_thumb_requests;
    assert!(asked >= 1);
    assert!(m.item("landscape/stored.dds").unwrap().thumb.is_none());
    for _ in 0..2 {
        m.replan();
        assert!(m.wait_idle(Duration::from_secs(60)));
        wait_thumbs(&mut m);
    }
    assert_eq!(m.dds_thumb_requests, asked, "a refused DDS was read again");
    // A changed file (new size, so a new stamp) is tried again.
    red_dds(&p, Format::Bc1, 128, 8);
    m.replan();
    assert!(m.wait_idle(Duration::from_secs(60)));
    wait_thumbs(&mut m);
    assert!(
        m.dds_thumb_requests > asked,
        "a changed DDS was not retried"
    );
}

/// `textures/pbr/eye` under `dir`, with the diffuse `eye.png`.
fn eye_folder(dir: &Path) -> std::path::PathBuf {
    let eye = dir.join("textures/pbr/eye");
    std::fs::create_dir_all(&eye).unwrap();
    image::ImageBuffer::from_fn(64, 64, |x, y| {
        image::Rgb([(x * 4) as u8, (y * 4) as u8, 90u8])
    })
    .save(eye.join("eye.png"))
    .unwrap();
    eye
}

/// Opens `dir/textures` as the app and polls (30 s) until an item with a job and a thumbnail has `key`.
fn opened_with_thumb(dir: &Path, key: impl Fn(&str) -> bool) -> sdc_gui::model::AppModel {
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    let end = Instant::now() + Duration::from_secs(30);
    while !m
        .items
        .iter()
        .any(|i| key(&i.key) && i.thumb.is_some() && i.job.is_some())
        && Instant::now() < end
    {
        m.poll();
        std::thread::sleep(Duration::from_millis(10));
    }
    m
}

/// Synthetic pre-packed eye fixture with low specular in alpha.
fn opened_with_eye(dir: &Path) -> sdc_gui::model::AppModel {
    let eye = eye_folder(dir);
    image::ImageBuffer::from_fn(64, 64, |_, _| image::Rgba([140u8, 0, 255, 10]))
        .save(eye.join("eye_rmaos.png"))
        .unwrap();
    opened_with_thumb(dir, |k| k.ends_with("eye_rmaos.png"))
}

/// A grayscale PNG of one value, the source of one packed channel.
fn gray_png(path: std::path::PathBuf, v: u8) {
    image::ImageBuffer::from_fn(64, 64, |_, _| image::Luma([v]))
        .save(path)
        .unwrap();
}

/// The eye's four channel sources (roughness 140, metallic 0, ao 255, specular 10), packed by the app.
fn opened_with_generated_eye(dir: &Path) -> sdc_gui::model::AppModel {
    let eye = eye_folder(dir);
    for (name, v) in [
        ("roughness", 140),
        ("metallic", 0),
        ("ao", 255),
        ("specular", 10),
    ] {
        gray_png(eye.join(format!("eye_{name}.png")), v);
    }
    opened_with_thumb(dir, |k| k == "pbr/eye/eye_rmaos")
}

/// What `ThumbCache::get` uploads for `item`, as the first whole-image upload of the frame after it.
fn uploaded(item: &sdc_gui::model::items::TextureItem) -> eframe::egui::ColorImage {
    let ctx = eframe::egui::Context::default();
    let mut cache = sdc_gui::ui::thumbs::ThumbCache::default();
    let tex = cache.get(&ctx, item).expect("a texture");
    let mut out = ctx.run_ui(Default::default(), |_| {});
    let id = tex.id();
    let upload = out.textures_delta.set.remove(&id);
    out.textures_delta.clear(); // also consumes the font upload and any frees
    let updates = upload.expect("thumbnail texture ID in this frame");
    let delta = updates
        .into_iter()
        .find(|d| d.pos.is_none())
        .expect("whole thumbnail upload");
    let eframe::egui::ImageData::Color(img) = delta.image;
    let img = std::sync::Arc::try_unwrap(img).unwrap_or_else(|a| (*a).clone());
    assert!(!img.pixels.is_empty(), "empty upload for {id:?}");
    img
}

fn mean_rgb(img: &eframe::egui::ColorImage) -> u32 {
    img.pixels
        .iter()
        .map(|c| c.r() as u32 + c.g() as u32 + c.b() as u32)
        .sum::<u32>()
        / (3 * img.pixels.len() as u32)
}

#[test]
fn a_packed_tile_is_not_drawn_black() {
    let d = tempfile::tempdir().unwrap();
    let m = opened_with_eye(d.path());
    let item = m
        .items
        .iter()
        .find(|i| i.key.ends_with("eye_rmaos.png"))
        .unwrap();
    assert_eq!(item.kind, sdc_gui::model::items::ItemKind::Image);
    assert_eq!(
        item.job.as_ref().unwrap().profile.alpha.policy,
        engine::config::AlphaPolicy::Data
    );
    let px = item.thumb.as_ref().expect("the tile has a thumbnail");
    let mean_a = px.rgba.chunks(4).map(|c| c[3] as u32).sum::<u32>() / (px.rgba.len() as u32 / 4);
    assert!(
        mean_a < 32,
        "premise: the alpha is low ({mean_a}); if not, stop and report the channel means"
    );
    // What the tile draws: columns.rs asks ThumbCache::get, which uploads this texture.
    let lum = mean_rgb(&uploaded(item));
    assert!(
        lum > 40,
        "drawn dark: upload mean rgb {lum}, thumbnail mean alpha {mean_a}"
    );
}

#[test]
fn the_packed_tile_draws_its_own_thumbnail() {
    let d = tempfile::tempdir().unwrap();
    let m = opened_with_generated_eye(d.path());
    let key = "pbr/eye/eye_rmaos";
    let item = m
        .items
        .iter()
        .find(|i| i.kind == sdc_gui::model::items::ItemKind::Packed && i.key == key)
        .expect("generated eye output, extensionless input-relative key");
    assert_eq!(item.name, "eye_rmaos");
    // The CS RMAOS recipe: R roughness, G metallic, B ao, A specular, each read from its own source file.
    let pack = item
        .job
        .as_ref()
        .unwrap()
        .pack
        .as_ref()
        .expect("a pack job");
    for (c, (src, want)) in pack
        .channels
        .iter()
        .zip([
            "eye_roughness.png",
            "eye_metallic.png",
            "eye_ao.png",
            "eye_specular.png",
        ])
        .enumerate()
    {
        let engine::pack::ChannelSource::Source { path, .. } = src else {
            panic!("channel {c} is not read from a source: {src:?}");
        };
        assert_eq!(
            path.file_name().and_then(|n| n.to_str()),
            Some(want),
            "channel {c} source"
        );
    }
    let thumb = item.thumb.as_ref().expect("assembled thumbnail");
    let n = thumb.rgba.len() as u32 / 4;
    let means: Vec<u32> = (0..4)
        .map(|c| {
            thumb
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| p[c] as u32)
                .sum::<u32>()
                / n
        })
        .collect();
    for (got, want) in means.iter().zip([140u32, 0, 255, 10]) {
        assert!(
            got.abs_diff(want) <= 2,
            "thumbnail rgba means {means:?}, wanted about [140, 0, 255, 10]"
        );
    }
    use eframe::egui;
    use egui_kittest::kittest::Queryable;
    let name = format!("thumb:{key}");
    let wanted = name.clone();
    let uploads = std::rc::Rc::new(std::cell::RefCell::new(std::collections::HashMap::new()));
    let collected = uploads.clone();
    let mut h = egui_kittest::Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            move |ui, m: &mut sdc_gui::model::AppModel| {
                sdc_gui::ui::draw(ui, m);
                let manager = ui.ctx().tex_manager();
                let mut frame = manager.write().take_delta();
                let sets = std::mem::take(&mut frame.set);
                frame.clear();
                for (id, updates) in sets {
                    if manager
                        .read()
                        .meta(id)
                        .is_some_and(|meta| meta.name == wanted)
                    {
                        for update in updates.into_iter().filter(|d| d.pos.is_none()) {
                            let egui::ImageData::Color(img) = update.image;
                            collected.borrow_mut().insert(id, img);
                        }
                    }
                }
            },
            m,
        );
    h.run();
    let tile = h
        .query_all_by_label("eye_rmaos")
        .find(|n| !in_sets_column(&h, n.rect()))
        .expect("the tile")
        .rect();
    let column =
        sdc_gui::ui::panel_ids::panel_rect(&h.ctx, "textures").expect("the Textures column");
    assert!(
        column.contains_rect(tile),
        "the tile {tile:?} is in the Textures column {column:?}"
    );
    let id = support::shapes(&h)
        .iter()
        .filter(|s| {
            let rect = s.visual_bounding_rect();
            let gap = tile.top() - rect.bottom();
            rect.contains(egui::pos2(tile.center().x, rect.center().y))
                && (0.0..=h.ctx.global_style().spacing.item_spacing.y + 2.0).contains(&gap)
        })
        .map(|s| s.texture_id())
        .find(|&id| {
            h.ctx
                .tex_manager()
                .read()
                .meta(id)
                .is_some_and(|m| m.name == name)
        })
        .expect("the tile painted its own thumbnail");
    let uploads = uploads.borrow();
    let img = uploads
        .get(&id)
        .expect("whole upload for the painted tile ID");
    assert!(!img.pixels.is_empty(), "empty upload for {id:?}");
    assert!(
        img.pixels.iter().all(|p| p.a() == 255 && p.b() > 200),
        "drawn tile lost its data channels: upload mean rgb {}, first pixel {:?}",
        mean_rgb(img),
        img.pixels[0]
    );
}

/// Control: a colour map keeps its transparency in the tile, and `alpha_is_data` follows the resolved policy.
#[test]
fn a_blend_tile_keeps_its_low_alpha_and_the_flag_follows_the_policy() {
    use engine::config::AlphaPolicy;
    use sdc_gui::model::items::ItemKind;
    let d = tempfile::tempdir().unwrap();
    let m = opened_with_eye(d.path());
    let mut item = m
        .items
        .iter()
        .find(|i| i.key.ends_with("eye_rmaos.png"))
        .unwrap()
        .clone();
    assert!(item.alpha_is_data(), "Data policy");
    item.job.as_mut().unwrap().profile.alpha.policy = AlphaPolicy::Remove;
    assert!(item.alpha_is_data(), "Remove policy");
    for p in [
        AlphaPolicy::Blend,
        AlphaPolicy::Cutout,
        AlphaPolicy::CutoutBlend,
    ] {
        item.job.as_mut().unwrap().profile.alpha.policy = p;
        assert!(!item.alpha_is_data(), "{p:?} keeps its alpha");
    }
    item.job.as_mut().unwrap().profile.alpha.policy = AlphaPolicy::Blend;
    let lum = mean_rgb(&uploaded(&item));
    assert!(lum < 20, "a Blend tile is still premultiplied: {lum}");
    item.job = None;
    assert!(!item.alpha_is_data(), "no job: the original transparency");
    item.kind = ItemKind::Packed;
    assert!(item.alpha_is_data(), "a packed item with no job");
}
