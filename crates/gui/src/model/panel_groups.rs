//! The texture panel's collapsible groups, which group each field sits in, and the open state kept in `gui.toml`
//! (texture-panel-compact plan, Task 1). No egui types.
use super::fields::CUBE_FIELDS;
use crate::settings::GuiSettings;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PanelGroup {
    Alpha,
    Mipmaps,
    NormalMap,
    Size,
    Compression,
    Cubemap,
    Channels,
    Sources,
}

impl PanelGroup {
    /// In panel order.
    pub const ALL: [PanelGroup; 8] = [
        PanelGroup::Alpha,
        PanelGroup::Mipmaps,
        PanelGroup::NormalMap,
        PanelGroup::Size,
        PanelGroup::Compression,
        PanelGroup::Cubemap,
        PanelGroup::Channels,
        PanelGroup::Sources,
    ];

    /// The key in `[texture_groups]` and the salt of the header's id.
    pub fn id(self) -> &'static str {
        match self {
            PanelGroup::Alpha => "alpha",
            PanelGroup::Mipmaps => "mipmaps",
            PanelGroup::NormalMap => "normal-map",
            PanelGroup::Size => "size",
            PanelGroup::Compression => "compression",
            PanelGroup::Cubemap => "cubemap",
            PanelGroup::Channels => "channels",
            PanelGroup::Sources => "sources",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            PanelGroup::Alpha => "Alpha",
            PanelGroup::Mipmaps => "Mipmaps",
            PanelGroup::NormalMap => "Normal map",
            PanelGroup::Size => "Size",
            PanelGroup::Compression => "Compression",
            PanelGroup::Cubemap => "Cubemap",
            PanelGroup::Channels => "Channels",
            PanelGroup::Sources => "Where these settings come from",
        }
    }
}

/// Whether a group is open when `gui.toml` says nothing about it.
pub fn default_open(g: PanelGroup) -> bool {
    // shortcut: every group but Sources starts open until Task 8 closes them, when this becomes `false` for all.
    g != PanelGroup::Sources
}

/// The rows that stay on the first screen, outside any group.
pub const FIRST_SCREEN: [&str; 2] = ["subtype", "format"];

/// The group a panel field sits in; `None` for a first-screen row or a key no panel lists.
pub fn panel_group(field: &str) -> Option<PanelGroup> {
    if CUBE_FIELDS.iter().any(|(k, _)| *k == field) {
        return Some(PanelGroup::Cubemap);
    }
    Some(match field {
        "channels.msn_source" | "mips.normal" | "mips.normal_base" => PanelGroup::NormalMap,
        "mips.coverage" | "encoder.bc1_alpha" => PanelGroup::Alpha,
        _ => match field.split('.').next().unwrap_or("") {
            "alpha" => PanelGroup::Alpha,
            "mips" => PanelGroup::Mipmaps,
            "resize" | "resize_relative_to" | "resize_floor" => PanelGroup::Size,
            "color" | "encoder" | "header" => PanelGroup::Compression,
            _ => return None,
        },
    })
}

pub fn is_open(settings: &GuiSettings, g: PanelGroup) -> bool {
    settings
        .texture_groups
        .get(g.id())
        .copied()
        .unwrap_or_else(|| default_open(g))
}

pub fn set_open(settings: &mut GuiSettings, g: PanelGroup, open: bool) {
    settings.texture_groups.insert(g.id().to_string(), open);
}
