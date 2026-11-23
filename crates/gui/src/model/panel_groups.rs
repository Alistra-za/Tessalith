//! The texture panel's collapsible groups, which group each field sits in, and the open state kept in `gui.toml`
//! (texture-panel-compact plan, Task 1). No egui types.
use super::fields::{
    cube_views, edge_view, exr_straight_view, field_hidden, field_views, filter_view, levels_view,
    mips_on, no_effect_reason, FieldView, BC3_SEARCH, CUBE_FIELDS, EDGE_FIELD, EFFORT_FIELDS,
    EXR_STRAIGHT_FIELD, FIELDS, FILTER_FIELD, LEVELS_FIELD, MSN_SOURCE, SEARCH_FIELDS, WEIGHTING,
};
use crate::settings::GuiSettings;
use crate::ui::settings_panel::option_label;
use engine::config::{Profile, Subtype, Target};
use engine::policy::ResolvedJob;
use serde_json::Value;

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
    let listed = |list: &[(&str, &str)]| list.iter().any(|(k, _)| *k == field);
    if FIRST_SCREEN.contains(&field) {
        return None;
    }
    if listed(&CUBE_FIELDS) {
        return Some(PanelGroup::Cubemap);
    }
    if listed(&[LEVELS_FIELD, FILTER_FIELD, EDGE_FIELD]) {
        return Some(PanelGroup::Mipmaps);
    }
    if listed(&[EXR_STRAIGHT_FIELD]) {
        return Some(PanelGroup::Alpha);
    }
    // The Effort row's search fields and the BC3 Effort row are Compression rows no `FIELDS` entry lists.
    if listed(&SEARCH_FIELDS) || field == BC3_SEARCH {
        return Some(PanelGroup::Compression);
    }
    // Only a key `FIELDS` lists is placed, so a field added there without a line here has no group and the table
    // test in `tests/texture_groups.rs` fails. Channels holds read-only lines, never a field.
    if !listed(&FIELDS) {
        return None;
    }
    match field {
        MSN_SOURCE | "mips.normal" | "mips.normal_base" => Some(PanelGroup::NormalMap),
        "alpha.policy" | "alpha.threshold" | "alpha.compare" | "mips.coverage"
        | "encoder.bc1_alpha" | "alpha.exact_opaque" | "alpha.parallax" => Some(PanelGroup::Alpha),
        "resize" | "resize_relative_to" | "resize_floor" => Some(PanelGroup::Size),
        "color.tag" | "color.content" | "encoder.effort" | WEIGHTING | "encoder.perceptual"
        | "header" => Some(PanelGroup::Compression),
        _ => None,
    }
}

/// One group's header text: the summary, an optional hover, and the two marks. Built once per input change, never per
/// frame (Task 4 caches it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupSummary {
    pub text: String,
    /// Why the group is not used, when every shown row is locked.
    pub hover: Option<String>,
    /// A row of the group holds a value this texture's own entry set.
    pub changed: bool,
    /// Every shown row is locked (`FieldView.lock`, never `edit_off`).
    pub all_locked: bool,
}

/// What `summary` reads besides the views: the Channels lines are read-only text no field carries.
pub struct SummaryExtra<'a> {
    /// The R, G, B and A sources of a packed texture.
    pub channels: Option<[&'a str; 4]>,
}

/// Every row the panel's groups show for `job`: `field_views`, the Levels, Filter and Edge views, the EXR Straight
/// alpha view, and `cube_views` on a cube map, without the rows `fields::hidden_reason` leaves out.
pub fn panel_views(target: Target, job: &ResolvedJob) -> Vec<FieldView> {
    let mut views = field_views(target, job);
    views.push(levels_view(target, job));
    views.push(filter_view(target, job));
    views.push(edge_view(target, job));
    views.extend(exr_straight_view(target, job));
    if job.profile.subtype == Subtype::Cubemap {
        views.extend(cube_views(target, job));
    }
    views.retain(|v| !field_hidden(&job.profile, v.field));
    views
}

const NOT_USED: &str = "not used for this texture";

fn shown(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The header summary of `g` over its rows in `views` (the texture panel's one-line value; the table is the plan's
/// "Header summaries"). A locked row is left out; a group whose every row is locked reads "not used for this texture".
pub fn summary(
    g: PanelGroup,
    views: &[FieldView],
    target: Target,
    p: &Profile,
    extra: &SummaryExtra<'_>,
) -> GroupSummary {
    let fixed = |text: String| GroupSummary {
        text,
        hover: None,
        changed: false,
        all_locked: false,
    };
    match g {
        PanelGroup::Sources => return fixed("How each value was chosen".into()),
        PanelGroup::Channels => {
            return fixed(extra.channels.map_or_else(String::new, |[r, g, b, a]| {
                format!("R {r}, G {g}, B {b}, A {a}")
            }));
        }
        _ => {}
    }
    let rows: Vec<&FieldView> = views
        .iter()
        .filter(|v| panel_group(v.field) == Some(g))
        .collect();
    let changed = rows.iter().any(|v| v.edited);
    if !rows.is_empty() && rows.iter().all(|v| v.lock.is_some()) {
        let first = rows[0].lock;
        let hover = if rows.iter().all(|v| v.lock == first) {
            format!("Every row here: {}.", first.unwrap_or_default())
        } else {
            "Each row below says why it is not used.".to_string()
        };
        return GroupSummary {
            text: NOT_USED.into(),
            hover: Some(hover),
            changed,
            all_locked: true,
        };
    }
    // The row of `field` unless it is locked; its option label, else the value as written.
    let row = |field: &str| rows.iter().find(|v| v.field == field && v.lock.is_none());
    let label = |v: &FieldView| {
        option_label(v.field, target, p, &v.value).unwrap_or_else(|| shown(&v.value))
    };
    let named = |field: &str| row(field).map(|v| label(v));
    let join =
        |parts: Vec<Option<String>>| parts.into_iter().flatten().collect::<Vec<_>>().join(", ");
    let text = match g {
        PanelGroup::Alpha => {
            let policy = row("alpha.policy").map(|v| {
                let cut = matches!(v.value.as_str(), Some("cutout" | "cutout_blend"));
                match (cut, row("alpha.threshold")) {
                    (true, Some(t)) => format!("{} at {}", label(v), shown(&t.value)),
                    _ => label(v),
                }
            });
            join(vec![policy])
        }
        PanelGroup::Mipmaps => match row(LEVELS_FIELD.0) {
            // The tick off writes "none": that option's own label, with no filter.
            Some(l) if !mips_on(&l.value) => label(l),
            levels => join(vec![
                named(FILTER_FIELD.0),
                levels.map(|l| label(l).to_lowercase()),
            ]),
        },
        PanelGroup::NormalMap => join(vec![
            named("mips.normal"),
            named(MSN_SOURCE).map(|b| format!("baked in {b}")),
        ]),
        PanelGroup::Size => match no_effect_reason(p, "resize") {
            Some(why) => why.to_string(),
            None => match row("resize") {
                Some(r) if r.value.as_str() == Some("none") => label(r),
                Some(r) => {
                    let of = row("resize_relative_to")
                        .filter(|v| v.value.as_str() != Some("self"))
                        .map(|v| format!(" of {}", label(v)))
                        .unwrap_or_default();
                    let floor = row("resize_floor")
                        .map(|v| format!(", floor {}", label(v)))
                        .unwrap_or_default();
                    format!("{}{of}{floor}", label(r))
                }
                None => String::new(),
            },
        },
        PanelGroup::Compression => join(vec![
            named("color.tag"),
            EFFORT_FIELDS
                .iter()
                .find_map(|f| row(f))
                .map(|v| format!("Effort {}", label(v))),
            row("header")
                .filter(|v| v.value.as_str() != Some("auto"))
                .map(|v| label(v)),
        ]),
        PanelGroup::Cubemap => join(vec![
            named("cubemap.layout"),
            row("cubemap.face_size").map(|v| match v.value.as_u64() {
                Some(n) => format!("{n} px faces"),
                None => "auto-sized faces".to_string(),
            }),
        ]),
        PanelGroup::Channels | PanelGroup::Sources => String::new(),
    };
    GroupSummary {
        text,
        hover: None,
        changed,
        all_locked: false,
    }
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
