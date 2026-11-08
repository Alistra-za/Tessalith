//! kittest pins for the Sets column polish, Note HN, on the driver's fixtures.
use engine::config::RnmTarget as R;
use engine::helper::{AlphaTarget as A, FillTarget as F};
use sdc_gui::model::helpers::{docked, fill_channel, fill_slot};
use sdc_gui::model::items::{helper_target_text, ItemKind};
use sdc_gui::model::sets::{self, SlotFill, SlotKind};

#[test]
fn fill_targets_map_to_the_chip_they_fill() {
    for (t, slot, ch) in [
        (F::Alpha(A::Diffuse), Some(SlotKind::Diffuse), "alpha"),
        (F::Alpha(A::N), Some(SlotKind::Normal), "alpha"),
        (F::Alpha(A::F), Some(SlotKind::Fuzz), "alpha"),
        (F::Alpha(A::S), Some(SlotKind::Subsurface), "alpha"),
        (F::Alpha(A::Cnr), Some(SlotKind::Coat), "alpha"),
        (
            F::Packed {
                suffix: "rmaos",
                channel: 0,
            },
            Some(SlotKind::Rmaos),
            "R",
        ),
        (
            F::Packed {
                suffix: "rfaos",
                channel: 3,
            },
            Some(SlotKind::Rmaos),
            "A",
        ),
        (
            F::Packed {
                suffix: "p",
                channel: 0,
            },
            Some(SlotKind::Height),
            "R",
        ),
        (
            F::Packed {
                suffix: "m",
                channel: 1,
            },
            None,
            "G",
        ),
        (F::Rnm(R::Cnr), Some(SlotKind::Coat), "layer"),
        (F::Rnm(R::N), Some(SlotKind::Normal), "layer"),
    ] {
        assert_eq!((fill_slot(t), fill_channel(t)), (slot, ch), "{t:?}");
    }
}

#[test]
fn the_helpers_fixture_has_a_two_chip_helper_a_filled_chip_helper_and_an_unused_one() {
    crate::app_data();
    let d = tempfile::tempdir().unwrap();
    let m = crate::fixtures::lookup("helpers").unwrap()(d.path());
    let rep = |k: &str| m.item(k).unwrap().helper.clone().unwrap();
    assert!(rep("props/crate_roughness.png").skipped_targets.is_empty());
    // sorted: the report order is the engine's
    let mut rough = rep("props/crate_roughness.png").fills.clone();
    rough.sort_by_key(|t| t.id());
    assert_eq!(
        rough,
        [
            F::Alpha(A::Cnr),
            F::Packed {
                suffix: "rmaos",
                channel: 0
            }
        ]
    );
    assert_eq!(rep("props/crate_opacity.png").fills, [F::Alpha(A::Diffuse)]);
    assert_eq!(
        helper_target_text(&rep("props/crate_metallic.png")),
        "not used: no targets chosen"
    );
    let views = sets::views(&m);
    let v = views
        .iter()
        .find(|v| v.id == "props/crate")
        .expect("the crate set");
    for k in [
        "props/crate_roughness.png",
        "props/crate_opacity.png",
        "props/crate_metallic.png",
    ] {
        assert!(v.members.iter().any(|x| x == k), "{k} is a member");
    }
    // Decision 2: the RMAOS chip holds the packed output, which has no source file.
    let Some(SlotFill::Own { item }) = v.slots.get(&SlotKind::Rmaos) else {
        panic!("RMAOS chip")
    };
    assert_eq!(m.item(item).unwrap().kind, ItemKind::Packed);
    let mut got: Vec<_> = docked(&m, v)
        .into_iter()
        .map(|d| (d.helper, d.slot, d.channel))
        .collect();
    got.sort();
    assert_eq!(
        got,
        [
            ("props/crate_opacity.png".into(), SlotKind::Diffuse, "alpha"),
            ("props/crate_roughness.png".into(), SlotKind::Rmaos, "R"),
            ("props/crate_roughness.png".into(), SlotKind::Coat, "alpha"),
        ]
    );
}

#[test]
fn the_demos_iron_layers_dock_on_its_normal_chip() {
    let (_d, h) = crate::replay("demo", &[]);
    let m = &h.state().model;
    let views = sets::views(m);
    let v = views
        .iter()
        .find(|v| v.id == "armor/iron")
        .expect("iron's set");
    let mut got: Vec<_> = docked(m, v)
        .into_iter()
        .map(|d| (d.helper, d.slot, d.channel))
        .collect();
    got.sort();
    assert_eq!(
        got,
        [
            ("armor/iron_bump.png".into(), SlotKind::Normal, "layer"),
            (
                "armor/iron_detailnorm.png".into(),
                SlotKind::Normal,
                "layer"
            ),
        ]
    );
}
