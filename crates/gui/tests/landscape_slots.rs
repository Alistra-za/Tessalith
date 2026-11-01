//! Note GI (XPMF Task 3, design X12, X16): the Projected noise and Detail normal slots of landscape sets. Neither
//! is a PGPatcher slot, so neither reaches the PGPatcher JSON or the CS TXST files.
mod support;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use engine::sets::OutputKey;
use engine::shader::{Shader, ShaderOrigin, ShaderPick};
use image::{ImageBuffer, Luma, Rgb};
use sdc_gui::model::cs_pbr::Kind;
use sdc_gui::model::sets::{
    engine_inputs, set_views, slot_row, usable_slots, SetView, SlotFill, SlotKind,
    DETAIL_NORMAL_HOVER,
};
use sdc_gui::model::AppModel;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

const ID: &str = "landscape/rock01";

fn view(m: &AppModel) -> SetView {
    set_views(m).into_iter().find(|v| v.id == ID).unwrap()
}

/// `support::pbr_opened` (rock01 auto PBR Landscape, on CS) with the extra files written first: `noise` a grey
/// map, `detail` a flat normal, each under rock01's name.
fn opened(dir: &Path, noise: Option<&str>, detail: Option<&str>) -> AppModel {
    let r = dir.join("textures/landscape");
    std::fs::create_dir_all(&r).unwrap();
    if let Some(n) = noise {
        ImageBuffer::from_fn(64, 64, |x, y| Luma([((x + y) * 2) as u8]))
            .save(r.join(n))
            .unwrap();
    }
    if let Some(n) = detail {
        ImageBuffer::from_pixel(64, 64, Rgb([128u8, 128, 255]))
            .save(r.join(n))
            .unwrap();
    }
    let mut m = support::pbr_opened(dir);
    m.set_cs_edids(ID, Kind::Txst, &["LandscapeRock01".into()]);
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
    assert_eq!(view(&m).shader.unwrap().shader, Shader::PbrLandscape);
    m
}

fn filled(v: &SetView, k: SlotKind) -> bool {
    matches!(v.slots.get(&k), Some(SlotFill::Own { .. }))
}

#[test]
fn landscape_set_shows_noise_and_detail_slots() {
    let new = [SlotKind::ProjectedNoise, SlotKind::DetailNormal];
    for s in [Shader::PbrLandscape, Shader::Landscape] {
        let u = usable_slots(s);
        assert!(new.iter().all(|k| u.contains(k)), "{s:?}: {u:?}");
    }
    let pbr = usable_slots(Shader::Pbr);
    assert!(new.iter().all(|k| !pbr.contains(k)), "{pbr:?}");
    assert_eq!(SlotKind::ProjectedNoise.label(), "Projected noise");
    assert_eq!(SlotKind::DetailNormal.label(), "Detail normal");
    assert_eq!(SlotKind::ProjectedNoise.suffix(), "noise");
    assert_eq!(SlotKind::DetailNormal.suffix(), "detail");
    assert!(new.iter().all(|k| k.slot_number().is_none()));
    // Two cards of their own (not one card with a clash), after the PGPatcher slots.
    let pick = ShaderPick {
        shader: Shader::PbrLandscape,
        provenance: ShaderOrigin::Fallback,
        wanted: None,
    };
    let own = |i: &str| SlotFill::Own { item: i.into() };
    let slots: BTreeMap<SlotKind, SlotFill> = [
        (SlotKind::Diffuse, own("d")),
        (SlotKind::ProjectedNoise, own("noise")),
        (SlotKind::DetailNormal, own("detail")),
    ]
    .into_iter()
    .collect();
    let row = slot_row(Some(&pick), &slots);
    let kinds: Vec<SlotKind> = row.iter().map(|c| c.kind).collect();
    assert_eq!(&kinds[kinds.len() - 2..], new, "{kinds:?}");
    assert!(row.iter().all(|c| c.clash.is_none() && c.usable));
}

/// Both slots filled: the PGPatcher entry and the CS TXST files equal the same set's without them.
#[test]
fn new_slots_stay_out_of_pgpatcher_json() {
    use sdc_gui::model::{cs_pbr_export, pbr_export};
    let files = |m: &AppModel| {
        let cs = cs_pbr_export::prepare(m).unwrap();
        let pgp: Vec<_> = pbr_export::json_entries(m)
            .into_iter()
            .map(|(_, e)| e)
            .collect();
        (cs.files, cs.notes, pgp)
    };
    let plain = tempfile::tempdir().unwrap();
    let plain = files(&opened(plain.path(), None, None));
    let d = tempfile::tempdir().unwrap();
    let m = opened(
        d.path(),
        Some("rock01_noise.png"),
        Some("rock01_detail.png"),
    );
    let v = view(&m);
    assert!(filled(&v, SlotKind::ProjectedNoise), "{:?}", v.slots);
    assert!(filled(&v, SlotKind::DetailNormal), "{:?}", v.slots);
    let with = files(&m);
    assert!(!with.0.is_empty(), "a TXST file is written");
    assert_eq!(with.2.len(), 1, "one PGPatcher entry");
    assert_eq!(with, plain);
}

/// On CS a PBR set's prefix names its outputs by `SlotKind::suffix()`: `rock01_detailnorm.png` in the Detail
/// normal slot writes `rock01_detail.dds`.
#[test]
fn cs_detail_output_name() {
    let d = tempfile::tempdir().unwrap();
    let m = opened(d.path(), None, Some("rock01_detailnorm.png"));
    assert!(filled(&view(&m), SlotKind::DetailNormal));
    let paths = engine_inputs(&m).1;
    let p = paths
        .get(&OutputKey::File("landscape/rock01_detailnorm.png".into()))
        .unwrap_or_else(|| panic!("no explicit path: {paths:?}"));
    assert_eq!(p.file_name().unwrap(), "rock01_detail.dds", "{p:?}");
}

/// X16: the Detail normal card of a landscape set says CS does not draw it in this build.
#[test]
fn detail_normal_card_says_cs_does_not_draw_it() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path(), None, Some("rock01_detail.png"));
    let key = view(&m).key;
    m.select_set(&key);
    let mut h = Harness::builder()
        .with_size(eframe::egui::vec2(1400.0, 2400.0))
        .build_ui_state(|ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.ctx.global_style_mut(|st| {
        st.interaction.tooltip_delay = 0.0;
        st.interaction.show_tooltips_only_when_still = false;
    });
    h.run();
    assert!(h
        .query_all_by_label_contains(DETAIL_NORMAL_HOVER)
        .next()
        .is_none());
    // The editor's card title is the right-most "Detail normal" node.
    let title = h
        .query_all_by_label("Detail normal")
        .map(|n| n.rect())
        .max_by(|a, b| a.left().total_cmp(&b.left()))
        .expect("the Detail normal card");
    h.hover_at(title.center());
    h.run();
    h.run();
    assert!(h
        .query_all_by_label_contains(DETAIL_NORMAL_HOVER)
        .next()
        .is_some());
}
