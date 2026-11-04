mod support;
use engine::config::Target;
use sdc_gui::model::preview::*;
use sdc_gui::model::*;
use std::time::{Duration, Instant};
use support::*;

fn settle(m: &mut AppModel) {
    assert!(m.wait_idle(Duration::from_secs(120)));
}

#[test]
fn encoding_is_off_by_default_and_encode_now_encodes_once() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    assert!(!m.preview.live);
    m.expand("landscape/rock01_rmaos");
    settle(&mut m);
    assert!(matches!(m.preview.source, SourceState::Ready(_)));
    assert!(matches!(m.preview.encoded, EncodedState::NotEncoded));
    m.tick(Instant::now() + Duration::from_secs(5));
    settle(&mut m);
    assert!(
        matches!(m.preview.encoded, EncodedState::NotEncoded),
        "live off: nothing encodes by itself"
    );
    m.encode_now();
    settle(&mut m);
    let EncodedState::Done(out) = &m.preview.encoded else {
        panic!("not done")
    };
    assert_eq!(out.result.source.first_level, 0);
    assert_eq!(out.decoded.len(), out.result.levels.len());
    // L12: with the heatmap off nothing decodes until a readout reads a mip.
    assert!(!m.view.heatmap);
    assert_eq!(out.decoded.decoded_count(), 0);
    m.preview_texel(0, (0, 0));
    assert_eq!(out.decoded.decoded_count(), 1);
}

#[test]
fn an_edit_makes_the_result_stale_and_live_re_encodes_after_the_debounce() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.expand("landscape/rock01_rmaos");
    m.encode_now();
    settle(&mut m);
    m.set_field(
        "landscape/rock01_rmaos",
        "encoder.effort",
        serde_json::json!("fast"),
    );
    settle(&mut m);
    assert!(matches!(m.preview.encoded, EncodedState::Stale(_)));
    m.set_live(true);
    let t0 = Instant::now();
    m.tick(t0);
    assert!(
        !matches!(m.preview.encoded, EncodedState::Encoding { .. }),
        "debounced"
    );
    m.tick(t0 + DEBOUNCE + Duration::from_millis(1));
    assert!(matches!(
        m.preview.encoded,
        EncodedState::Encoding { previous: Some(_) }
    ));
    settle(&mut m);
    assert!(matches!(m.preview.encoded, EncodedState::Done(_)));
}

#[test]
fn mode_change_cancels_and_drops_a_preview_of_a_vanished_job() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let set = m
        .item("landscape/rock01_rmaos")
        .unwrap()
        .set_key
        .clone()
        .unwrap();
    m.expand("landscape/rock01_rmaos");
    m.encode_now();
    // Switch target while the encode may still be running: vanilla builds no _rmaos.
    m.set_target(Target::Vanilla);
    settle(&mut m);
    assert!(m.item("landscape/rock01_rmaos").is_none());
    assert_eq!(m.expanded, None);
    assert_eq!(m.preview.item, None);
    assert!(matches!(m.preview.encoded, EncodedState::NotEncoded));
    assert_eq!(m.selection, Selection::Set(set));
}

#[test]
fn the_size_limit_skips_large_mips() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.gpu_max_dim = 16;
    m.expand("landscape/rock01_rmaos");
    m.encode_now();
    settle(&mut m);
    let EncodedState::Done(out) = &m.preview.encoded else {
        panic!("not done")
    };
    assert_eq!(out.result.source.first_level, 2, "64 and 32 are above 16");
}

#[test]
fn items_without_a_job_preview_nothing() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.expand("landscape/rock01_roughness.png");
    assert_eq!(m.preview.notice.as_deref(), Some("Nothing to preview"));
    assert!(!m.busy(), "nothing was started");
    // An unresolved job previews its current settings.
    m.expand("landscape/leaf.png");
    assert_eq!(
        m.preview.notice.as_deref(),
        Some("Preview of the suggested settings")
    );
    settle(&mut m);
    assert!(matches!(m.preview.source, SourceState::Ready(_)));
}

#[test]
fn a_dds_input_previews_its_own_bytes() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    export_through(&mut m);
    settle(&mut m);
    let dds = walk(&d.path().join("export"))
        .into_iter()
        .find(|p| p.extension().is_some_and(|e| e == "dds"))
        .expect("the export wrote a .dds");
    let root = d.path().join("textures/landscape");
    std::fs::copy(&dds, root.join("stored.dds")).unwrap();
    m.open_folder(d.path().join("textures"));
    settle(&mut m);
    m.expand("landscape/stored.dds");
    settle(&mut m);
    let EncodedState::Done(out) = &m.preview.encoded else {
        panic!("not done: {:?}", m.preview.encoded)
    };
    assert!(!out.result.levels.is_empty());
    assert_eq!(out.decoded.len(), out.result.levels.len());
    assert!(out.result.source.levels.is_empty(), "no source side");
    assert_eq!(
        m.preview.notice.as_deref(),
        Some("DDS input: no source image")
    );
}

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = vec![];
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

#[test]
fn a_result_for_a_collapsed_texture_is_dropped() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.expand("landscape/rock01_rmaos");
    m.encode_now();
    m.collapse();
    assert!(!m.busy(), "collapsing cancels the work");
    assert!(
        m.poll_until_stale_drop(StaleKind::Encode, Duration::from_secs(30)),
        "the collapsed texture's reply never came"
    );
    assert_eq!(m.preview.item, None);
    assert!(matches!(m.preview.encoded, EncodedState::NotEncoded));
}

#[test]
fn live_encode_is_saved_and_encodes_on_expand() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let path = d.path().join("gui.toml");
    m.settings_path = Some(path.clone());
    m.set_live(true);
    assert!(sdc_gui::settings::GuiSettings::load(&path).0.live_encode);
    m.expand("landscape/rock01_rmaos");
    settle(&mut m);
    assert!(matches!(m.preview.encoded, EncodedState::NotEncoded));
    let t0 = Instant::now();
    m.tick(t0);
    m.tick(t0 + DEBOUNCE);
    assert!(matches!(m.preview.encoded, EncodedState::Encoding { .. }));
    settle(&mut m);
    assert!(matches!(m.preview.encoded, EncodedState::Done(_)));
}

#[test]
fn the_gpu_limit_lowers_the_preview_limit() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    assert_eq!(m.preview_limit(), 4096, "a constant 4096 on a big GPU");
    m.gpu_max_dim = 8192;
    assert_eq!(m.preview_limit(), 4096, "a bigger GPU does not raise it");
    m.gpu_max_dim = 32;
    assert_eq!(m.preview_limit(), 32);
    m.expand("landscape/rock01_rmaos");
    settle(&mut m);
    let SourceState::Ready(src) = &m.preview.source else {
        panic!("no source")
    };
    assert_eq!(src.first_level, 1, "64 px does not fit 32 px");
    assert_eq!(
        m.preview.notice.as_deref(),
        Some("This GPU shows at most 32 px: previewing from the first mip that fits")
    );
    m.encode_now();
    settle(&mut m);
    let EncodedState::Done(out) = &m.preview.encoded else {
        panic!("not done")
    };
    assert_eq!(out.result.source.first_level, 1);
}

#[test]
fn a_gpu_limit_that_changes_nothing_adds_no_notice() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.gpu_max_dim = 1024;
    m.expand("landscape/rock01_rmaos");
    settle(&mut m);
    assert!(m.preview.notice.is_none(), "{:?}", m.preview.notice);
}

/// Writes a 16x16 full-chain DDS of `pf` with every texel `px` (raw level bytes per texel).
fn raw_dds(path: &std::path::Path, dxgi: u32, bpp: u32, texel: &[u8]) {
    let mut levels = vec![];
    let mut s = 16u32;
    loop {
        levels.extend(
            std::iter::repeat_n(texel, (s * s) as usize)
                .flatten()
                .copied(),
        );
        if s == 1 {
            break;
        }
        s /= 2;
    }
    let mut hd = [0u32; 31];
    hd[0] = 124;
    hd[1] = 0x1 | 0x2 | 0x4 | 0x8 | 0x1000 | 0x2_0000;
    hd[2] = 16;
    hd[3] = 16;
    hd[4] = 16 * bpp;
    hd[5] = 1;
    hd[6] = 5;
    hd[18] = 32;
    hd[19] = 0x4;
    hd[20] = u32::from_le_bytes(*b"DX10");
    hd[26] = 0x1000 | 0x8 | 0x40_0000;
    let mut b = b"DDS ".to_vec();
    for v in hd {
        b.extend_from_slice(&v.to_le_bytes());
    }
    for v in [dxgi, 3, 0, 1, 0] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend(levels);
    std::fs::write(path, b).unwrap();
}

/// DECISIONS S5: N11 ends. sdc's own rgba8 and a third-party BGRX8 preview from the CPU decode.
#[test]
fn uncompressed_dds_inputs_preview_from_the_cpu_decode() {
    for (name, dxgi, bpp, texel, want) in [
        (
            "own.dds",
            87u32,
            4u32,
            vec![30u8, 20, 10, 40],
            [10.0f32, 20.0, 30.0, 40.0],
        ),
        (
            "third.dds",
            88,
            4,
            vec![30, 20, 10, 0],
            [10.0, 20.0, 30.0, 255.0],
        ),
    ] {
        let d = tempfile::tempdir().unwrap();
        let root = support::mod_folder(d.path());
        raw_dds(&root.join("landscape").join(name), dxgi, bpp, &texel);
        let mut m = opened(d.path());
        m.expand(&format!("landscape/{name}"));
        settle(&mut m);
        let EncodedState::Done(out) = &m.preview.encoded else {
            panic!("{name}: {:?}", m.preview.encoded)
        };
        let px = out.decoded_mip(0).unwrap()[0];
        for c in 0..4 {
            assert!((px[c] * 255.0 - want[c]).abs() < 0.5, "{name} c{c}: {px:?}");
        }
    }
}

/// The red code of a tile of a 16x16 float DDS whose texels are `[v, -3, 0, 1]`; also checks the pane's format.
fn float_tile_red(f32s: bool, v: f32) -> u8 {
    let texel: Vec<u8> = if f32s {
        [v, -3.0, 0.0, 1.0]
            .iter()
            .flat_map(|c| c.to_le_bytes())
            .collect()
    } else {
        [v, -3.0, 0.0, 1.0]
            .iter()
            .flat_map(|c| half::f16::from_f32(*c).to_le_bytes())
            .collect()
    };
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("hdr.dds");
    raw_dds(
        &p,
        if f32s { 2 } else { 10 },
        if f32s { 16 } else { 8 },
        &texel,
    );
    // The main preview reads it as an HDR format, so the shader tone-maps it.
    assert!(dds_preview(&p, 64).unwrap().result.format.is_hdr());
    let t = dds_thumbnail(&p).unwrap();
    // Negative green is 0 before the map; alpha is untouched.
    assert_eq!((t.rgba[1], t.rgba[3]), (0, 255), "f32 {f32s}");
    t.rgba[0]
}

/// Note FM31: an HDR DDS tile maps `x / (1 + x)` then sRGB, so values above 1
/// stay apart; an LDR tile keeps its stored codes.
#[test]
fn hdr_preview_tone_maps_values_above_one() {
    for f32s in [true, false] {
        let codes: Vec<u8> = [1.0, 4.0, 16.0]
            .iter()
            .map(|v| float_tile_red(f32s, *v))
            .collect();
        // x / (1 + x) gives 0.5, 0.8, 0.941; sRGB-encoded: 188, 231, 248.
        assert_eq!(codes, [188, 231, 248], "f32 {f32s}");
    }
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("ldr.dds");
    raw_dds(&p, 87, 4, &[30u8, 20, 200, 40]);
    let t = dds_thumbnail(&p).unwrap();
    assert_eq!(t.rgba[..4], [200, 20, 30, 40], "a BGRA8 tile is as stored");
}

/// Fix round 1: a 4x4 BC6H UF16 DDS (one mip, one mode-3 block: 10-bit endpoints, every texel `v10` in all three
/// channels). Whatever float that decodes to is above 1, which the test checks.
fn bc6h_dds(path: &std::path::Path, v10: u128) {
    let block: u128 = 3 | (v10 << 5) | (v10 << 15) | (v10 << 25);
    let mut hd = [0u32; 31];
    hd[0] = 124;
    hd[1] = 0x1 | 0x2 | 0x4 | 0x1000 | 0x8_0000;
    hd[2] = 4;
    hd[3] = 4;
    hd[4] = 16;
    hd[5] = 1;
    hd[6] = 1;
    hd[18] = 32;
    hd[19] = 0x4;
    hd[20] = u32::from_le_bytes(*b"DX10");
    hd[26] = 0x1000;
    let mut b = b"DDS ".to_vec();
    for v in hd {
        b.extend_from_slice(&v.to_le_bytes());
    }
    for v in [95u32, 3, 0, 1, 0] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(&block.to_le_bytes());
    std::fs::write(path, b).unwrap();
}

/// Fix round 1: a BC6H tile is tone-mapped too (the decode's float is above 1, so a clamp would give 255).
#[test]
fn a_bc6h_tile_is_tone_mapped() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("bc6h.dds");
    bc6h_dds(&p, 548);
    let out = dds_preview(&p, 64).unwrap();
    assert!(out.result.format.is_hdr());
    let x = out.decoded_mip(0).unwrap()[0][0];
    assert!(x > 1.5 && x.is_finite(), "decoded {x}");
    let m = x / (1.0 + x);
    let want = 255.0 * (1.055 * m.powf(1.0 / 2.4) - 0.055);
    let tile = dds_thumbnail(&p).unwrap();
    assert!(tile.rgba[0] < 255, "not clamped: {:?}", &tile.rgba[..4]);
    assert!(
        (f32::from(tile.rgba[0]) - want).abs() <= 1.0,
        "{:?} vs {want}",
        &tile.rgba[..4]
    );
}

/// Trace #32: Encode now during an export says it waits, so it encodes once the export is done (Live encode off).
#[test]
fn encode_now_during_an_export_runs_when_the_export_ends() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.expand("landscape/rock01_rmaos");
    settle(&mut m);
    m.export.running = Some(Default::default());
    m.encode_now();
    let t0 = Instant::now();
    m.tick(t0 + DEBOUNCE * 2);
    assert!(
        matches!(m.preview.encoded, EncodedState::NotEncoded),
        "waits while the export runs"
    );
    assert!(m.preview.notice.is_some(), "the notice says it waits");
    m.export.running = None;
    m.tick(t0 + DEBOUNCE * 3);
    m.tick(t0 + DEBOUNCE * 5);
    settle(&mut m);
    assert!(matches!(m.preview.encoded, EncodedState::Done(_)));
    assert!(m.preview.notice.is_none(), "and goes once it has run");
}

/// The labels of the expanded preview's toolbar controls, sorted.
fn toolbar_controls(m: AppModel) -> (Vec<String>, Vec<(String, eframe::egui::Pos2)>) {
    use eframe::egui::{self, accesskit::Role, containers::panel::PanelState};
    use egui_kittest::kittest::{NodeT, Queryable};
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1400.0, 900.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    let bar = PanelState::load(&h.ctx, egui::Id::new("preview-toolbar"))
        .expect("the toolbar is drawn")
        .outer_rect;
    let mut out = vec![];
    for role in [
        Role::Button,
        Role::CheckBox,
        Role::SpinButton,
        Role::Slider,
        Role::ComboBox,
    ] {
        for n in h.query_all_by_role(role) {
            if bar.contains_rect(n.rect()) {
                if let Some(l) = n.accesskit_node().label() {
                    out.push(l);
                }
            }
        }
    }
    out.sort();
    // The pane captions: where the source and the encoded pane start.
    let mut panes = vec![];
    for c in &h.output().shapes {
        if let egui::Shape::Text(t) = &c.shape {
            if matches!(t.galley.text(), "Source" | "Encoded") {
                panes.push((t.galley.text().to_string(), t.pos));
            }
        }
    }
    (out, panes)
}

/// egui-ui-rules: at the doc-shot and narrower widths no toolbar control is cut off by the panel's edge (the flow
/// wraps whole widgets).
#[test]
fn no_toolbar_control_is_clipped_at_narrow_widths() {
    use eframe::egui::{self, accesskit::Role, containers::panel::PanelState};
    use egui_kittest::kittest::{NodeT, Queryable};
    for width in [960.0, 1100.0, 1280.0, 1600.0] {
        let d = tempfile::tempdir().unwrap();
        let mut m = opened(d.path());
        m.expand("landscape/rock01_rmaos");
        settle(&mut m);
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(width, 810.0))
            .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
        sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
        h.run();
        let bar = PanelState::load(&h.ctx, egui::Id::new("preview-toolbar"))
            .expect("the toolbar is drawn")
            .outer_rect;
        for role in [Role::Button, Role::CheckBox, Role::Slider] {
            for n in h.query_all_by_role(role) {
                let r = n.rect();
                let inside_row =
                    bar.x_range().contains(r.min.x) && r.min.y >= bar.min.y && r.max.y <= bar.max.y;
                if inside_row {
                    assert!(
                        r.max.x <= bar.max.x + 0.5,
                        "{width}: {:?} {r:?} runs past the toolbar {bar:?}",
                        n.accesskit_node().label()
                    );
                }
            }
        }
    }
}

/// Note FM26: the preview offers the side-by-side pair with Heatmap, Lens, Tile 3×3 and UV coverage, and the HDR
/// exposure only for an HDR texture. Pin, Compare, Split and the Range overlay are gone.
#[test]
fn preview_offers_side_by_side_heatmap_lens_tile_and_exposure_only() {
    let base = [
        "1:1",
        "A",
        "B",
        "Encode now",
        "Fit",
        "G",
        "Heatmap",
        "Lens",
        "Live encode",
        "R",
        "RGBA",
        "Tile 3×3",
        "UNORM",
        "UV coverage overlay",
        "sRGB",
        "α-mask",
        "✕ Close",
    ];
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.expand("landscape/rock01_rmaos");
    settle(&mut m);
    m.encode_now();
    settle(&mut m);
    let (got, panes) = toolbar_controls(m);
    assert_eq!(got, base);
    // Side by side is the only layout: the Source pane starts left of the Encoded one, on the same row.
    let at = |name: &str| panes.iter().find(|(n, _)| n == name).map(|(_, p)| *p);
    let (src, enc) = (
        at("Source").expect("Source pane"),
        at("Encoded").expect("Encoded pane"),
    );
    assert!(enc.x > src.x + 100.0, "{src:?} {enc:?}");
    assert_eq!(src.y, enc.y);

    // An HDR texture adds the exposure slider and nothing else (the exact set, so a dropped or new control shows).
    let d = tempfile::tempdir().unwrap();
    let root = support::mod_folder(d.path());
    raw_dds(
        &root.join("landscape").join("sky.dds"),
        10,
        8,
        &[0, 0x3C, 0, 0x3C, 0, 0x3C, 0, 0x3C],
    );
    let mut m = opened(d.path());
    m.expand("landscape/sky.dds");
    settle(&mut m);
    let (got, _) = toolbar_controls(m);
    let mut want: Vec<&str> = base.to_vec();
    want.push("Exposure");
    want.sort();
    assert_eq!(got, want);
}
