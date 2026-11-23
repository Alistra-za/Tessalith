//! The texture panel's group list and their open state (texture-panel-compact plan, Task 1).
use sdc_gui::model::panel_groups::{self, PanelGroup};

#[test]
fn every_panel_field_has_one_place() {
    for (key, _) in sdc_gui::model::bulk::panel_fields() {
        let first = panel_groups::FIRST_SCREEN.contains(&key);
        assert!(first != panel_groups::panel_group(key).is_some(), "{key}");
    }
    assert_eq!(
        panel_groups::panel_group("channels.msn_source"),
        Some(PanelGroup::NormalMap)
    );
    assert_eq!(
        panel_groups::panel_group("mips.normal"),
        Some(PanelGroup::NormalMap)
    );
    assert_eq!(
        panel_groups::panel_group("mips.normal_base"),
        Some(PanelGroup::NormalMap)
    );
    assert_eq!(
        panel_groups::panel_group("mips.filter"),
        Some(PanelGroup::Mipmaps)
    );
    assert_eq!(
        panel_groups::panel_group("mips.coverage"),
        Some(PanelGroup::Alpha)
    );
    assert_eq!(
        panel_groups::panel_group("header"),
        Some(PanelGroup::Compression)
    );
    assert_eq!(panel_groups::panel_group("format"), None);
    for (key, _) in sdc_gui::model::fields::CUBE_FIELDS {
        assert_eq!(
            panel_groups::panel_group(key),
            Some(PanelGroup::Cubemap),
            "{key}"
        );
    }
}

/// The plan's field-to-group table, one line per field (plan "Where each row goes"). A field that lands in another
/// group, or one that no line lists, fails here.
#[test]
fn the_whole_field_to_group_table_is_pinned() {
    use PanelGroup::*;
    let table: [(&str, Option<PanelGroup>); 31] = [
        ("subtype", None),
        ("format", None),
        ("channels.green_convention", None),
        ("alpha.policy", Some(Alpha)),
        ("alpha.threshold", Some(Alpha)),
        ("alpha.compare", Some(Alpha)),
        ("mips.coverage", Some(Alpha)),
        ("encoder.bc1_alpha", Some(Alpha)),
        ("alpha.exact_opaque", Some(Alpha)),
        ("alpha.parallax", Some(Alpha)),
        ("alpha.exr_straight", Some(Alpha)),
        ("mips.levels", Some(Mipmaps)),
        ("mips.filter", Some(Mipmaps)),
        ("mips.edge", Some(Mipmaps)),
        ("mips.normal", Some(NormalMap)),
        ("mips.normal_base", Some(NormalMap)),
        ("channels.msn_source", Some(NormalMap)),
        ("resize", Some(Size)),
        ("resize_relative_to", Some(Size)),
        ("resize_floor", Some(Size)),
        ("color.tag", Some(Compression)),
        ("color.content", Some(Compression)),
        ("encoder.effort", Some(Compression)),
        ("encoder.bc7_weights", Some(Compression)),
        ("encoder.perceptual", Some(Compression)),
        ("header", Some(Compression)),
        ("encoder.bc3_search", Some(Compression)),
        ("encoder.bc45_search_radius", Some(Compression)),
        ("encoder.bc1_search", Some(Compression)),
        // Channels holds read-only lines, never a field; an unknown key has no group.
        ("channels.red", None),
        ("no.such_field", None),
    ];
    for (key, want) in table {
        assert_eq!(panel_groups::panel_group(key), want, "{key}");
    }
    let listed: Vec<&str> = table.iter().map(|(k, _)| *k).collect();
    for (key, _) in sdc_gui::model::bulk::panel_fields()
        .into_iter()
        .chain(sdc_gui::model::fields::FIELDS)
        .chain(sdc_gui::model::fields::all_fields())
    {
        assert!(listed.contains(&key), "{key} has no line in the table");
    }
}

#[test]
fn a_stored_state_overrides_the_default_and_an_old_file_takes_defaults() {
    let mut s = sdc_gui::settings::GuiSettings::default();
    panel_groups::set_open(&mut s, PanelGroup::Sources, true);
    assert!(panel_groups::is_open(&s, PanelGroup::Sources));
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("gui.toml");
    std::fs::write(&p, "live_encode = true\n").unwrap();
    let (old, err) = sdc_gui::settings::GuiSettings::load(&p);
    assert_eq!(err, None);
    for g in PanelGroup::ALL {
        assert_eq!(
            panel_groups::is_open(&old, g),
            panel_groups::default_open(g)
        );
    }
}

#[test]
fn group_ids_are_unique_and_titles_follow_the_table() {
    let ids: std::collections::BTreeSet<_> = PanelGroup::ALL.iter().map(|g| g.id()).collect();
    assert_eq!(ids.len(), 8);
    assert_eq!(PanelGroup::NormalMap.title(), "Normal map");
    assert_eq!(
        PanelGroup::Sources.title(),
        "Where these settings come from"
    );
}

#[test]
fn where_settings_come_from_starts_closed_from_the_start() {
    let s = sdc_gui::settings::GuiSettings::default();
    assert!(!panel_groups::is_open(&s, PanelGroup::Sources));
}
