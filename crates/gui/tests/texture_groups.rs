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
